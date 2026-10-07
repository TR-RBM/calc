use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};

use calc_expr::{AccessError, BinderKind, ExprId, ExprPool, Head, NodeView, Operator, SymbolKind};
use calc_numbers::{Integer, Number};
use calc_units::UnitId;

use crate::exact_evaluation::exact_sign;

const DEFAULT_CAP: u32 = 5;
const LARGEST_CAP: u32 = 100;
const DEFAULT_WORK_BUDGET: u64 = 100_000;
const LARGEST_WORK_BUDGET: u64 = 10_000_000;
const SMALLEST_SPECIFICITY: u64 = 2;
const SMALLEST_SPECIFICITY_OF_A_WHOLE_MATCH: u64 = 1;
const COVERAGE_DENOMINATOR_FACTOR: usize = 2;
const RENAMING_PLACEHOLDER_BASE: u32 = 0xF0000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecognitionPattern {
    pub concept: String,
    pub identifier: String,
    pub parameters: usize,
    pub function: ExprId,
    pub conditions: Vec<ExprId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecognitionLimits {
    pub cap: u32,
    pub work_budget: u64,
}

impl Default for RecognitionLimits {
    fn default() -> Self {
        Self {
            cap: DEFAULT_CAP,
            work_budget: DEFAULT_WORK_BUDGET,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecognitionTruncation {
    Cap,
    WorkBudget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecognizedMatch {
    pub concept: String,
    pub pattern: String,
    pub coverage: Number,
    pub specificity: u64,
    pub site: ExprId,
    pub bindings: Vec<ExprId>,
    pub holds: Vec<Option<bool>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternRecognition {
    pub matches: Vec<RecognizedMatch>,
    pub truncated: bool,
    pub truncated_by: Option<RecognitionTruncation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecognitionError {
    CapOutOfRange { cap: u32 },
    WorkBudgetOutOfRange { work_budget: u64 },
    NotAPatternFunction { pattern: String },
    Access(AccessError),
}

#[derive(Clone)]
struct State {
    bindings: Vec<Option<ExprId>>,
    covered: BTreeSet<ExprId>,
}

struct Budget {
    used: u64,
    limit: u64,
    exhausted: bool,
}

impl Budget {
    fn spend(&mut self) -> bool {
        if self.used >= self.limit {
            self.exhausted = true;
            return false;
        }
        self.used += 1;
        true
    }
}

fn chain_operator(head: Head) -> Option<Operator> {
    match head {
        Head::Operator(operator @ (Operator::Add | Operator::Mul)) => Some(operator),
        _ => None,
    }
}

fn opened_body(pool: &ExprPool, function: ExprId, parameters: usize) -> Option<ExprId> {
    let mut body = function;
    for _ in 0..parameters {
        match pool.node(body).ok()? {
            NodeView::Bind {
                binder: BinderKind::Lambda,
                body: inner,
                ..
            } => body = inner,
            _ => return None,
        }
    }
    Some(body)
}

fn children(pool: &ExprPool, expression: ExprId) -> Result<Vec<ExprId>, AccessError> {
    Ok(match pool.node(expression)? {
        NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_) => Vec::new(),
        NodeView::Apply { arguments, .. } => arguments.to_vec(),
        NodeView::Bind {
            arguments, body, ..
        } => arguments.iter().copied().chain([body]).collect(),
        NodeView::Quantity { value, .. } => vec![value],
        NodeView::Array { elements, .. } => elements.to_vec(),
    })
}

fn pre_order(pool: &ExprPool, root: ExprId) -> Result<Vec<ExprId>, AccessError> {
    let mut order = Vec::new();
    let mut seen = HashSet::new();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !seen.insert(expression) {
            continue;
        }
        order.push(expression);
        pending.extend(children(pool, expression)?.into_iter().rev());
    }
    Ok(order)
}

struct BindPattern<'pattern> {
    binder: BinderKind,
    arguments: &'pattern [ExprId],
    body: ExprId,
}

struct Matcher<'pool> {
    pool: &'pool ExprPool,
    parameters: usize,
    loose: HashMap<ExprId, u32>,
    equivalent: HashMap<(ExprId, ExprId), bool>,
}

impl Matcher<'_> {
    fn loose_bound(&mut self, expression: ExprId) -> u32 {
        if let Some(loose) = self.loose.get(&expression) {
            return *loose;
        }
        let loose = match self.pool.node(expression) {
            Ok(NodeView::Bound(index)) => index.saturating_add(1),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                let arguments = arguments.to_vec();
                let from_arguments = arguments
                    .into_iter()
                    .map(|argument| self.loose_bound(argument))
                    .max()
                    .unwrap_or(0);
                from_arguments.max(self.loose_bound(body).saturating_sub(1))
            }
            Ok(_) => children(self.pool, expression)
                .unwrap_or_default()
                .into_iter()
                .map(|child| self.loose_bound(child))
                .max()
                .unwrap_or(0),
            Err(_) => u32::MAX,
        };
        self.loose.insert(expression, loose);
        loose
    }

    fn variable(&self, index: u32, depth: u32) -> Option<usize> {
        let outer = usize::try_from(index.checked_sub(depth)?).ok()?;
        self.parameters.checked_sub(outer + 1)
    }

    fn chain(&self, expression: ExprId, operator: Operator) -> (Vec<ExprId>, Vec<ExprId>) {
        let mut nodes = Vec::new();
        let mut operands = Vec::new();
        let mut pending = vec![expression];
        while let Some(current) = pending.pop() {
            match self.pool.node(current) {
                Ok(NodeView::Apply { head, arguments })
                    if chain_operator(head) == Some(operator) =>
                {
                    nodes.push(current);
                    pending.extend(arguments.iter().rev());
                }
                _ => operands.push(current),
            }
        }
        (nodes, operands)
    }

    fn cover_subtree(&self, covered: &mut BTreeSet<ExprId>, expression: ExprId) {
        let mut pending = vec![expression];
        while let Some(current) = pending.pop() {
            if covered.insert(current) {
                pending.extend(children(self.pool, current).unwrap_or_default());
            }
        }
    }

    fn are_equivalent(&mut self, left: ExprId, right: ExprId, budget: &mut Budget) -> bool {
        if left == right {
            return true;
        }
        if let Some(known) = self.equivalent.get(&(left, right)) {
            return *known;
        }
        let result = match (self.pool.node(left), self.pool.node(right)) {
            (
                Ok(NodeView::Apply {
                    head: left_head,
                    arguments: left_arguments,
                }),
                Ok(NodeView::Apply {
                    head: right_head,
                    arguments: right_arguments,
                }),
            ) if left_head == right_head => match chain_operator(left_head) {
                Some(operator) => {
                    let (_, left_operands) = self.chain(left, operator);
                    let (_, right_operands) = self.chain(right, operator);
                    left_operands.len() == right_operands.len()
                        && self.pair_equivalent(
                            &left_operands,
                            &right_operands,
                            &mut vec![false; right_operands.len()],
                            budget,
                        )
                }
                None => {
                    let pairs: Vec<(ExprId, ExprId)> = left_arguments
                        .iter()
                        .copied()
                        .zip(right_arguments.iter().copied())
                        .collect();
                    left_arguments.len() == right_arguments.len()
                        && pairs
                            .into_iter()
                            .all(|(left, right)| self.are_equivalent(left, right, budget))
                }
            },
            (
                Ok(NodeView::Quantity {
                    value: left_value,
                    unit: left_unit,
                }),
                Ok(NodeView::Quantity {
                    value: right_value,
                    unit: right_unit,
                }),
            ) => left_unit == right_unit && self.are_equivalent(left_value, right_value, budget),
            (
                Ok(NodeView::Bind {
                    binder: left_binder,
                    arguments: left_arguments,
                    body: left_body,
                }),
                Ok(NodeView::Bind {
                    binder: right_binder,
                    arguments: right_arguments,
                    body: right_body,
                }),
            ) if left_binder == right_binder && left_arguments.len() == right_arguments.len() => {
                let pairs: Vec<(ExprId, ExprId)> = left_arguments
                    .iter()
                    .copied()
                    .zip(right_arguments.iter().copied())
                    .chain([(left_body, right_body)])
                    .collect();
                pairs
                    .into_iter()
                    .all(|(left, right)| self.are_equivalent(left, right, budget))
            }
            (
                Ok(NodeView::Array {
                    shape: left_shape,
                    elements: left_elements,
                }),
                Ok(NodeView::Array {
                    shape: right_shape,
                    elements: right_elements,
                }),
            ) if left_shape == right_shape => {
                let pairs: Vec<(ExprId, ExprId)> = left_elements
                    .iter()
                    .copied()
                    .zip(right_elements.iter().copied())
                    .collect();
                pairs
                    .into_iter()
                    .all(|(left, right)| self.are_equivalent(left, right, budget))
            }
            _ => false,
        };
        self.equivalent.insert((left, right), result);
        result
    }

    fn pair_equivalent(
        &mut self,
        left: &[ExprId],
        right: &[ExprId],
        used: &mut Vec<bool>,
        budget: &mut Budget,
    ) -> bool {
        let Some((first, rest)) = left.split_first() else {
            return true;
        };
        for position in 0..right.len() {
            if used[position] {
                continue;
            }
            if !budget.spend() {
                return false;
            }
            if self.are_equivalent(*first, right[position], budget) {
                used[position] = true;
                let found = self.pair_equivalent(rest, right, used, budget);
                used[position] = false;
                if found {
                    return true;
                }
            }
        }
        false
    }

    fn match_node(
        &mut self,
        pattern: ExprId,
        depth: u32,
        subject: ExprId,
        state: State,
        budget: &mut Budget,
    ) -> Vec<State> {
        if budget.exhausted {
            return Vec::new();
        }
        let Ok(pattern_view) = self.pool.node(pattern) else {
            return Vec::new();
        };
        match pattern_view {
            NodeView::Bound(index) => match self.variable(index, depth) {
                Some(variable) => self.bind(variable, subject, state, budget),
                None => self.same_leaf(pattern, subject, state),
            },
            NodeView::Number(_) | NodeView::Symbol(_) => self.same_leaf(pattern, subject, state),
            NodeView::Apply { head, arguments } => {
                let arguments = arguments.to_vec();
                match chain_operator(head) {
                    Some(operator) => {
                        self.match_chain(pattern, operator, depth, subject, state, budget)
                    }
                    None => self.match_apply(head, &arguments, depth, subject, state, budget),
                }
            }
            NodeView::Quantity { value, unit } => {
                self.match_quantity(value, unit, depth, subject, state, budget)
            }
            NodeView::Bind {
                binder,
                arguments,
                body,
            } => {
                let arguments = arguments.to_vec();
                let pattern = BindPattern {
                    binder,
                    arguments: &arguments,
                    body,
                };
                self.match_bind(pattern, depth, subject, state, budget)
            }
            NodeView::Array { shape, elements } => {
                let shape = shape.to_vec();
                let elements = elements.to_vec();
                self.match_array(&shape, &elements, depth, subject, state, budget)
            }
        }
    }

    fn same_leaf(&self, pattern: ExprId, subject: ExprId, mut state: State) -> Vec<State> {
        if pattern != subject {
            return Vec::new();
        }
        state.covered.insert(subject);
        vec![state]
    }

    fn bind(
        &mut self,
        variable: usize,
        subject: ExprId,
        mut state: State,
        budget: &mut Budget,
    ) -> Vec<State> {
        if self.loose_bound(subject) > 0 {
            return Vec::new();
        }
        match state.bindings[variable] {
            None => state.bindings[variable] = Some(subject),
            Some(bound) => {
                if !self.are_equivalent(bound, subject, budget) {
                    return Vec::new();
                }
            }
        }
        self.cover_subtree(&mut state.covered, subject);
        vec![state]
    }

    fn match_sequence(
        &mut self,
        pairs: &[(ExprId, ExprId)],
        depths: &[u32],
        states: Vec<State>,
        budget: &mut Budget,
    ) -> Vec<State> {
        let mut states = states;
        for ((pattern, subject), depth) in pairs.iter().zip(depths) {
            states = states
                .into_iter()
                .flat_map(|state| self.match_node(*pattern, *depth, *subject, state, budget))
                .collect();
            if states.is_empty() {
                break;
            }
        }
        states
    }

    fn same_operator_bindings(
        &mut self,
        pattern: ExprId,
        subject: ExprId,
        budget: &mut Budget,
    ) -> Option<Vec<Vec<ExprId>>> {
        let Ok(NodeView::Apply { head, arguments }) = self.pool.node(pattern) else {
            return Some(Vec::new());
        };
        let pattern_arguments = arguments.to_vec();
        let mut found = Vec::new();
        let mut pending = self.same_head_operands(subject, head);
        while let Some(node) = pending.pop() {
            pending.extend(self.same_head_operands(node, head));
            let start = State {
                bindings: vec![None; self.parameters],
                covered: BTreeSet::new(),
            };
            let bindings = self
                .match_apply(head, &pattern_arguments, 0, node, start, budget)
                .into_iter()
                .next()?
                .bindings
                .into_iter()
                .collect::<Option<Vec<_>>>()?;
            found.push(bindings);
        }
        Some(found)
    }

    fn same_head_operands(&self, node: ExprId, head: Head) -> Vec<ExprId> {
        let Ok(NodeView::Apply { arguments, .. }) = self.pool.node(node) else {
            return Vec::new();
        };
        arguments
            .iter()
            .copied()
            .filter(|argument| {
                matches!(
                    self.pool.node(*argument),
                    Ok(NodeView::Apply { head: argument_head, .. }) if argument_head == head
                )
            })
            .collect()
    }

    fn match_written_grouping(
        &mut self,
        pattern: ExprId,
        subject: ExprId,
        state: State,
        budget: &mut Budget,
    ) -> Vec<State> {
        let Ok(NodeView::Apply { head, arguments }) = self.pool.node(pattern) else {
            return Vec::new();
        };
        if chain_operator(head).is_none() {
            return Vec::new();
        }
        let arguments = arguments.to_vec();
        self.match_apply(head, &arguments, 0, subject, state, budget)
    }

    fn match_apply(
        &mut self,
        head: Head,
        arguments: &[ExprId],
        depth: u32,
        subject: ExprId,
        mut state: State,
        budget: &mut Budget,
    ) -> Vec<State> {
        let Ok(NodeView::Apply {
            head: subject_head,
            arguments: subject_arguments,
        }) = self.pool.node(subject)
        else {
            return Vec::new();
        };
        if subject_head != head || subject_arguments.len() != arguments.len() {
            return Vec::new();
        }
        let pairs: Vec<(ExprId, ExprId)> = arguments
            .iter()
            .copied()
            .zip(subject_arguments.iter().copied())
            .collect();
        state.covered.insert(subject);
        let depths = vec![depth; pairs.len()];
        self.match_sequence(&pairs, &depths, vec![state], budget)
    }

    fn match_quantity(
        &mut self,
        value: ExprId,
        unit: UnitId,
        depth: u32,
        subject: ExprId,
        mut state: State,
        budget: &mut Budget,
    ) -> Vec<State> {
        let Ok(NodeView::Quantity {
            value: subject_value,
            unit: subject_unit,
        }) = self.pool.node(subject)
        else {
            return Vec::new();
        };
        if subject_unit != unit {
            return Vec::new();
        }
        state.covered.insert(subject);
        self.match_node(value, depth, subject_value, state, budget)
    }

    fn match_bind(
        &mut self,
        pattern: BindPattern<'_>,
        depth: u32,
        subject: ExprId,
        mut state: State,
        budget: &mut Budget,
    ) -> Vec<State> {
        let BindPattern {
            binder,
            arguments,
            body,
        } = pattern;
        let Ok(NodeView::Bind {
            binder: subject_binder,
            arguments: subject_arguments,
            body: subject_body,
        }) = self.pool.node(subject)
        else {
            return Vec::new();
        };
        if subject_binder != binder || subject_arguments.len() != arguments.len() {
            return Vec::new();
        }
        let mut pairs: Vec<(ExprId, ExprId)> = arguments
            .iter()
            .copied()
            .zip(subject_arguments.iter().copied())
            .collect();
        let mut depths = vec![depth; pairs.len()];
        pairs.push((body, subject_body));
        depths.push(depth + 1);
        state.covered.insert(subject);
        self.match_sequence(&pairs, &depths, vec![state], budget)
    }

    fn match_array(
        &mut self,
        shape: &[u32],
        elements: &[ExprId],
        depth: u32,
        subject: ExprId,
        mut state: State,
        budget: &mut Budget,
    ) -> Vec<State> {
        let Ok(NodeView::Array {
            shape: subject_shape,
            elements: subject_elements,
        }) = self.pool.node(subject)
        else {
            return Vec::new();
        };
        if subject_shape != shape {
            return Vec::new();
        }
        let pairs: Vec<(ExprId, ExprId)> = elements
            .iter()
            .copied()
            .zip(subject_elements.iter().copied())
            .collect();
        state.covered.insert(subject);
        let depths = vec![depth; pairs.len()];
        self.match_sequence(&pairs, &depths, vec![state], budget)
    }

    fn match_chain(
        &mut self,
        pattern: ExprId,
        operator: Operator,
        depth: u32,
        subject: ExprId,
        mut state: State,
        budget: &mut Budget,
    ) -> Vec<State> {
        let is_same_chain = matches!(
            self.pool.node(subject),
            Ok(NodeView::Apply { head, .. }) if chain_operator(head) == Some(operator)
        );
        if !is_same_chain {
            return Vec::new();
        }
        let (_, pattern_operands) = self.chain(pattern, operator);
        let (subject_nodes, subject_operands) = self.chain(subject, operator);
        if pattern_operands.len() != subject_operands.len() {
            return Vec::new();
        }
        state.covered.extend(subject_nodes);
        let mut used = vec![false; subject_operands.len()];
        self.assign(
            &pattern_operands,
            &subject_operands,
            &mut used,
            depth,
            state,
            budget,
        )
    }

    fn assign(
        &mut self,
        pattern_operands: &[ExprId],
        subject_operands: &[ExprId],
        used: &mut Vec<bool>,
        depth: u32,
        state: State,
        budget: &mut Budget,
    ) -> Vec<State> {
        let Some((first, rest)) = pattern_operands.split_first() else {
            return vec![state];
        };
        let mut found = Vec::new();
        for position in 0..subject_operands.len() {
            if used[position] {
                continue;
            }
            if !budget.spend() {
                return found;
            }
            let matched = self.match_node(
                *first,
                depth,
                subject_operands[position],
                state.clone(),
                budget,
            );
            used[position] = true;
            for next in matched {
                found.extend(self.assign(rest, subject_operands, used, depth, next, budget));
            }
            used[position] = false;
        }
        found
    }

    fn specificity(&self, body: ExprId) -> u64 {
        let mut counted = HashSet::new();
        let mut pending = vec![(body, 0_u32)];
        let mut visited = HashSet::new();
        while let Some((expression, depth)) = pending.pop() {
            if !visited.insert((expression, depth)) {
                continue;
            }
            match self.pool.node(expression) {
                Ok(NodeView::Bound(index)) if self.variable(index, depth).is_some() => {}
                Ok(NodeView::Bind {
                    arguments, body, ..
                }) => {
                    counted.insert(expression);
                    pending.extend(arguments.iter().map(|argument| (*argument, depth)));
                    pending.push((body, depth + 1));
                }
                Ok(_) => {
                    counted.insert(expression);
                    pending.extend(
                        children(self.pool, expression)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|child| (child, depth)),
                    );
                }
                Err(_) => {}
            }
        }
        u64::try_from(counted.len()).unwrap_or(u64::MAX)
    }
}

fn substituted(
    pool: &mut ExprPool,
    expression: ExprId,
    depth: u32,
    parameters: usize,
    bindings: &[ExprId],
) -> Option<ExprId> {
    match pool.node(expression).ok()? {
        NodeView::Number(_) | NodeView::Symbol(_) => Some(expression),
        NodeView::Bound(index) => {
            let Some(outer) = index.checked_sub(depth) else {
                return Some(expression);
            };
            let position = parameters.checked_sub(usize::try_from(outer).ok()? + 1)?;
            bindings.get(position).copied()
        }
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            let replaced = arguments
                .into_iter()
                .map(|argument| substituted(pool, argument, depth, parameters, bindings))
                .collect::<Option<Vec<_>>>()?;
            pool.apply(head, &replaced).ok()
        }
        NodeView::Quantity { value, unit } => {
            let value = substituted(pool, value, depth, parameters, bindings)?;
            pool.quantity(value, unit).ok()
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments.to_vec();
            let replaced = arguments
                .into_iter()
                .map(|argument| substituted(pool, argument, depth, parameters, bindings))
                .collect::<Option<Vec<_>>>()?;
            let body = substituted(pool, body, depth + 1, parameters, bindings)?;
            pool.bind(binder, &replaced, body).ok()
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let replaced = elements
                .into_iter()
                .map(|element| substituted(pool, element, depth, parameters, bindings))
                .collect::<Option<Vec<_>>>()?;
            pool.array(&shape, &replaced).ok()
        }
    }
}

fn holds_a_machine_number(pool: &ExprPool, expression: ExprId) -> bool {
    let mut pending = vec![expression];
    let mut seen: Vec<ExprId> = Vec::new();
    while let Some(current) = pending.pop() {
        if seen.contains(&current) {
            continue;
        }
        seen.push(current);
        match pool.node(current) {
            Ok(NodeView::Number(number)) => {
                if matches!(
                    pool.number_value(number),
                    Ok(Number::F32(_) | Number::F64(_))
                ) {
                    return true;
                }
            }
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            Ok(NodeView::Array { elements, .. }) => pending.extend(elements.iter().copied()),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.extend(arguments.iter().copied());
                pending.push(body);
            }
            _ => {}
        }
    }
    false
}

fn decide(pool: &mut ExprPool, condition: ExprId) -> Option<bool> {
    let NodeView::Apply {
        head: Head::Operator(operator),
        arguments,
    } = pool.node(condition).ok()?
    else {
        return None;
    };
    let arguments = arguments.to_vec();
    match (operator, arguments.as_slice()) {
        (Operator::Not, [inner]) => decide(pool, *inner).map(|holds| !holds),
        (Operator::And, [left, right]) => match (decide(pool, *left), decide(pool, *right)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        (Operator::Or, [left, right]) => match (decide(pool, *left), decide(pool, *right)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        (
            Operator::Equal
            | Operator::NotEqual
            | Operator::Less
            | Operator::LessOrEqual
            | Operator::Greater
            | Operator::GreaterOrEqual,
            [left, right],
        ) => {
            if holds_a_machine_number(pool, *left) || holds_a_machine_number(pool, *right) {
                return None;
            }
            let difference = pool
                .apply(Head::Operator(Operator::Sub), &[*left, *right])
                .ok()?;
            let sign = exact_sign(pool, difference)?;
            Some(match operator {
                Operator::Equal => sign == Ordering::Equal,
                Operator::NotEqual => sign != Ordering::Equal,
                Operator::Less => sign == Ordering::Less,
                Operator::LessOrEqual => sign != Ordering::Greater,
                Operator::Greater => sign == Ordering::Greater,
                _ => sign != Ordering::Less,
            })
        }
        _ => None,
    }
}

struct Candidate {
    pattern: usize,
    site: usize,
    bindings: Vec<ExprId>,
    inner_bindings: Vec<Vec<ExprId>>,
    covered: usize,
}

struct Offered {
    pattern: usize,
    site: usize,
    bindings: Vec<ExprId>,
    covered: usize,
    specificity: u64,
    holds: Vec<Option<bool>>,
    binding_text: String,
}

fn checked_limits(limits: RecognitionLimits) -> Result<(), RecognitionError> {
    if !(1..=LARGEST_CAP).contains(&limits.cap) {
        return Err(RecognitionError::CapOutOfRange { cap: limits.cap });
    }
    if !(1..=LARGEST_WORK_BUDGET).contains(&limits.work_budget) {
        return Err(RecognitionError::WorkBudgetOutOfRange {
            work_budget: limits.work_budget,
        });
    }
    Ok(())
}

pub fn equivalent_pattern_shapes(
    pool: &mut ExprPool,
    left: &RecognitionPattern,
    right: &RecognitionPattern,
) -> Result<bool, RecognitionError> {
    let not_a_function = |pattern: &RecognitionPattern| RecognitionError::NotAPatternFunction {
        pattern: pattern.identifier.clone(),
    };
    let left_body =
        opened_body(pool, left.function, left.parameters).ok_or_else(|| not_a_function(left))?;
    let right_body =
        opened_body(pool, right.function, right.parameters).ok_or_else(|| not_a_function(right))?;
    if left.parameters != right.parameters {
        return Ok(false);
    }
    let mut placeholders = Vec::with_capacity(right.parameters);
    for position in 0..right.parameters {
        let offset = u32::try_from(position).unwrap_or(u32::MAX);
        let name = char::from_u32(RENAMING_PLACEHOLDER_BASE.saturating_add(offset))
            .map(String::from)
            .unwrap_or_default();
        let symbol = pool
            .intern_symbol(&name, SymbolKind::Variable)
            .map_err(|_| not_a_function(right))?;
        placeholders.push(pool.symbol(symbol).map_err(|_| not_a_function(right))?);
    }
    let Some(subject) = substituted(pool, right_body, 0, right.parameters, &placeholders) else {
        return Err(not_a_function(right));
    };
    let mut matcher = Matcher {
        pool,
        parameters: left.parameters,
        loose: HashMap::new(),
        equivalent: HashMap::new(),
    };
    let mut budget = Budget {
        used: 0,
        limit: LARGEST_WORK_BUDGET,
        exhausted: false,
    };
    let start = State {
        bindings: vec![None; left.parameters],
        covered: BTreeSet::new(),
    };
    let renames = matcher
        .match_node(left_body, 0, subject, start, &mut budget)
        .into_iter()
        .any(|state| {
            let bound: Option<Vec<ExprId>> = state.bindings.iter().copied().collect();
            bound.is_some_and(|bound| {
                let distinct: HashSet<ExprId> = bound.iter().copied().collect();
                distinct.len() == bound.len()
                    && bound.iter().all(|binding| placeholders.contains(binding))
            })
        });
    Ok(renames)
}

pub fn recognize(
    pool: &mut ExprPool,
    root: ExprId,
    patterns: &[RecognitionPattern],
    limits: RecognitionLimits,
    print: &dyn Fn(&ExprPool, ExprId) -> String,
) -> Result<PatternRecognition, RecognitionError> {
    checked_limits(limits)?;
    let bodies = patterns
        .iter()
        .map(|pattern| {
            opened_body(pool, pattern.function, pattern.parameters).ok_or_else(|| {
                RecognitionError::NotAPatternFunction {
                    pattern: pattern.identifier.clone(),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let condition_bodies = patterns
        .iter()
        .map(|pattern| {
            pattern
                .conditions
                .iter()
                .map(|condition| {
                    opened_body(pool, *condition, pattern.parameters).ok_or_else(|| {
                        RecognitionError::NotAPatternFunction {
                            pattern: pattern.identifier.clone(),
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sites = pre_order(pool, root).map_err(RecognitionError::Access)?;
    let total = sites.len();
    let mut order: Vec<usize> = (0..patterns.len()).collect();
    order.sort_by(|left, right| {
        (&patterns[*left].concept, &patterns[*left].identifier)
            .cmp(&(&patterns[*right].concept, &patterns[*right].identifier))
    });
    let mut budget = Budget {
        used: 0,
        limit: limits.work_budget,
        exhausted: false,
    };
    let mut candidates = Vec::new();
    let mut specificities = vec![0_u64; patterns.len()];
    {
        let mut matcher = Matcher {
            pool,
            parameters: 0,
            loose: HashMap::new(),
            equivalent: HashMap::new(),
        };
        for index in &order {
            matcher.parameters = patterns[*index].parameters;
            specificities[*index] = matcher.specificity(bodies[*index]);
        }
        'sites: for (site_position, site) in sites.iter().enumerate() {
            if matcher.loose_bound(*site) > 0 {
                continue;
            }
            for index in &order {
                matcher.parameters = patterns[*index].parameters;
                let start = State {
                    bindings: vec![None; patterns[*index].parameters],
                    covered: BTreeSet::new(),
                };
                let mut matched =
                    matcher.match_node(bodies[*index], 0, *site, start.clone(), &mut budget);
                let is_the_line = site_position == 0;
                if matched.is_empty() && is_the_line && specificities[*index] < SMALLEST_SPECIFICITY
                {
                    matched =
                        matcher.match_written_grouping(bodies[*index], *site, start, &mut budget);
                }
                let inner_bindings = if matched.is_empty()
                    || !is_the_line
                    || specificities[*index] >= SMALLEST_SPECIFICITY
                {
                    Some(Vec::new())
                } else {
                    matcher.same_operator_bindings(bodies[*index], *site, &mut budget)
                };
                let Some(inner_bindings) = inner_bindings else {
                    continue;
                };
                for state in matched {
                    let Some(bindings) = state.bindings.iter().copied().collect::<Option<Vec<_>>>()
                    else {
                        continue;
                    };
                    candidates.push(Candidate {
                        pattern: *index,
                        site: site_position,
                        bindings,
                        inner_bindings: inner_bindings.clone(),
                        covered: state.covered.len(),
                    });
                }
                if budget.exhausted {
                    break 'sites;
                }
            }
        }
    }
    let mut offered: Vec<Offered> = Vec::new();
    for candidate in candidates {
        let specificity = specificities[candidate.pattern];
        let is_covering_enough = candidate
            .covered
            .saturating_mul(COVERAGE_DENOMINATOR_FACTOR)
            >= total;
        let is_whole_match = specificity < SMALLEST_SPECIFICITY;
        if !is_covering_enough
            || specificity < SMALLEST_SPECIFICITY_OF_A_WHOLE_MATCH
            || (is_whole_match && candidate.covered < total)
        {
            continue;
        }
        let pattern = &patterns[candidate.pattern];
        let mut holds = Vec::new();
        for body in &condition_bodies[candidate.pattern] {
            if !budget.spend() {
                break;
            }
            let decided = substituted(pool, *body, 0, pattern.parameters, &candidate.bindings)
                .and_then(|condition| decide(pool, condition));
            holds.push(decided);
        }
        if budget.exhausted {
            break;
        }
        if holds.contains(&Some(false))
            || (is_whole_match && holds.iter().any(|holds| *holds != Some(true)))
        {
            continue;
        }
        let mut every_inner_node_holds = true;
        'inner: for inner in &candidate.inner_bindings {
            for body in &condition_bodies[candidate.pattern] {
                if !budget.spend() {
                    break 'inner;
                }
                let decided = substituted(pool, *body, 0, pattern.parameters, inner)
                    .and_then(|condition| decide(pool, condition));
                if decided != Some(true) {
                    every_inner_node_holds = false;
                    break 'inner;
                }
            }
        }
        if budget.exhausted {
            break;
        }
        if !every_inner_node_holds {
            continue;
        }
        let binding_text = candidate
            .bindings
            .iter()
            .map(|binding| print(pool, *binding))
            .collect::<Vec<_>>()
            .join("\u{0}");
        offered.push(Offered {
            pattern: candidate.pattern,
            site: candidate.site,
            bindings: candidate.bindings,
            covered: candidate.covered,
            specificity,
            holds,
            binding_text,
        });
    }
    offered.sort_by(|left, right| {
        let all_true = |offer: &Offered| offer.holds.iter().all(|holds| *holds == Some(true));
        let is_specific = |offer: &Offered| offer.specificity >= SMALLEST_SPECIFICITY;
        is_specific(right)
            .cmp(&is_specific(left))
            .then(right.covered.cmp(&left.covered))
            .then(right.specificity.cmp(&left.specificity))
            .then(all_true(right).cmp(&all_true(left)))
            .then(
                patterns[left.pattern]
                    .concept
                    .cmp(&patterns[right.pattern].concept),
            )
            .then(
                patterns[left.pattern]
                    .identifier
                    .cmp(&patterns[right.pattern].identifier),
            )
            .then(left.site.cmp(&right.site))
            .then(left.binding_text.cmp(&right.binding_text))
    });
    let mut kept = HashSet::new();
    offered.retain(|offer| kept.insert((offer.pattern, offer.site)));
    let cap = usize::try_from(limits.cap).unwrap_or(usize::MAX);
    let truncated_by = if budget.exhausted {
        Some(RecognitionTruncation::WorkBudget)
    } else if offered.len() > cap {
        Some(RecognitionTruncation::Cap)
    } else {
        None
    };
    let total_integer = Integer::from(u64::try_from(total).unwrap_or(u64::MAX));
    let matches = offered
        .into_iter()
        .take(cap)
        .map(|offer| {
            let pattern = &patterns[offer.pattern];
            let covered = Integer::from(u64::try_from(offer.covered).unwrap_or(u64::MAX));
            RecognizedMatch {
                concept: pattern.concept.clone(),
                pattern: pattern.identifier.clone(),
                coverage: Number::fraction(&covered, &total_integer)
                    .unwrap_or_else(|_| Number::from(0_i64)),
                specificity: offer.specificity,
                site: sites[offer.site],
                bindings: offer.bindings,
                holds: offer.holds,
            }
        })
        .collect();
    Ok(PatternRecognition {
        matches,
        truncated: truncated_by.is_some(),
        truncated_by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::{BuiltinConstant, ReductionShape};

    #[derive(Clone, Debug, PartialEq)]
    enum Token {
        Number(String),
        Machine(String),
        Name(String),
        Symbol(&'static str),
    }

    const SYMBOLS: [&str; 15] = [
        "|->", "==", ">=", "<=", "+", "-", "*", "/", "^", "(", ")", ",", ">", "<", "'",
    ];

    fn tokens(text: &str) -> Vec<Token> {
        let characters: Vec<char> = text.chars().collect();
        let mut found = Vec::new();
        let mut position = 0;
        while position < characters.len() {
            let character = characters[position];
            if character.is_whitespace() {
                position += 1;
            } else if character.is_ascii_digit() {
                let start = position;
                while position < characters.len()
                    && (characters[position].is_ascii_digit() || characters[position] == '.')
                {
                    position += 1;
                }
                found.push(Token::Number(characters[start..position].iter().collect()));
            } else if character.is_alphabetic() {
                let start = position;
                while position < characters.len()
                    && (characters[position].is_alphanumeric() || characters[position] == '_')
                {
                    position += 1;
                }
                let name: String = characters[start..position].iter().collect();
                if name == "f64" && characters.get(position) == Some(&'\'') {
                    let end = characters[position + 1..]
                        .iter()
                        .position(|character| *character == '\'')
                        .unwrap()
                        + position
                        + 1;
                    found.push(Token::Machine(
                        characters[position + 1..end].iter().collect(),
                    ));
                    position = end + 1;
                } else {
                    found.push(Token::Name(name));
                }
            } else {
                let rest: String = characters[position..].iter().collect();
                let symbol = SYMBOLS
                    .iter()
                    .find(|symbol| rest.starts_with(**symbol))
                    .unwrap_or_else(|| panic!("unexpected {rest}"));
                found.push(Token::Symbol(symbol));
                position += symbol.chars().count();
            }
        }
        found
    }

    struct TestParser<'pool> {
        pool: &'pool mut ExprPool,
        tokens: Vec<Token>,
        position: usize,
        scope: Vec<String>,
    }

    impl TestParser<'_> {
        fn peek(&self) -> Option<&Token> {
            self.tokens.get(self.position)
        }

        fn is_symbol(&self, symbol: &str) -> bool {
            matches!(self.peek(), Some(Token::Symbol(found)) if *found == symbol)
        }

        fn is_name(&self, name: &str) -> bool {
            matches!(self.peek(), Some(Token::Name(found)) if found == name)
        }

        fn expect(&mut self, symbol: &str) {
            assert!(self.is_symbol(symbol), "expected {symbol}");
            self.position += 1;
        }

        fn apply(&mut self, operator: Operator, arguments: &[ExprId]) -> ExprId {
            self.pool
                .apply(Head::Operator(operator), arguments)
                .unwrap()
        }

        fn lambda(&mut self) -> ExprId {
            let is_lambda = matches!(self.peek(), Some(Token::Name(_)))
                && matches!(
                    self.tokens.get(self.position + 1),
                    Some(Token::Symbol("|->"))
                );
            if !is_lambda {
                return self.conjunction();
            }
            let Some(Token::Name(name)) = self.peek().cloned() else {
                unreachable!()
            };
            self.position += 2;
            self.scope.push(name.clone());
            let body = self.lambda();
            self.scope.pop();
            let binder = self.pool.bind(BinderKind::Lambda, &[], body).unwrap();
            self.pool.record_bound_name(binder, &name).unwrap();
            binder
        }

        fn conjunction(&mut self) -> ExprId {
            let mut left = self.comparison();
            while self.is_name("and") {
                self.position += 1;
                let right = self.comparison();
                left = self.apply(Operator::And, &[left, right]);
            }
            left
        }

        fn comparison(&mut self) -> ExprId {
            let left = self.sum();
            let operator = match self.peek() {
                Some(Token::Symbol("==")) => Operator::Equal,
                Some(Token::Symbol(">")) => Operator::Greater,
                Some(Token::Symbol("<")) => Operator::Less,
                Some(Token::Symbol(">=")) => Operator::GreaterOrEqual,
                Some(Token::Symbol("<=")) => Operator::LessOrEqual,
                _ => return left,
            };
            self.position += 1;
            let right = self.sum();
            self.apply(operator, &[left, right])
        }

        fn sum(&mut self) -> ExprId {
            let mut left = self.product();
            loop {
                let operator = match self.peek() {
                    Some(Token::Symbol("+")) => Operator::Add,
                    Some(Token::Symbol("-")) => Operator::Sub,
                    _ => return left,
                };
                self.position += 1;
                let right = self.product();
                left = self.apply(operator, &[left, right]);
            }
        }

        fn product(&mut self) -> ExprId {
            let mut left = self.unary();
            loop {
                let operator = match self.peek() {
                    Some(Token::Symbol("*")) => Operator::Mul,
                    Some(Token::Symbol("/")) => Operator::Div,
                    _ => return left,
                };
                self.position += 1;
                let right = self.unary();
                left = self.apply(operator, &[left, right]);
            }
        }

        fn unary(&mut self) -> ExprId {
            if self.is_symbol("-") {
                self.position += 1;
                let operand = self.unary();
                return self.apply(Operator::Neg, &[operand]);
            }
            let base = self.primary();
            if self.is_symbol("^") {
                self.position += 1;
                let exponent = self.unary();
                return self.apply(Operator::Pow, &[base, exponent]);
            }
            base
        }

        fn number(&mut self, text: &str) -> ExprId {
            let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
            let digits: i128 = format!("{whole}{fraction}").parse().unwrap();
            let scale = Integer::from(10_i64).pow(u32::try_from(fraction.len()).unwrap());
            let value = Number::fraction(&Integer::from(digits), &scale).unwrap();
            self.pool.number(value).unwrap()
        }

        fn arguments(&mut self) -> Vec<ExprId> {
            self.expect("(");
            let mut found = vec![self.lambda()];
            while self.is_symbol(",") {
                self.position += 1;
                found.push(self.lambda());
            }
            self.expect(")");
            found
        }

        fn primary(&mut self) -> ExprId {
            let token = self.peek().cloned().expect("an operand");
            self.position += 1;
            match token {
                Token::Number(text) => {
                    let value = self.number(&text);
                    match self.peek().cloned() {
                        Some(Token::Name(unit)) if !self.scope.contains(&unit) => {
                            match self.pool.units_mut().lookup(&unit) {
                                Ok(unit) => {
                                    self.position += 1;
                                    self.pool.quantity(value, unit).unwrap()
                                }
                                Err(_) => value,
                            }
                        }
                        _ => value,
                    }
                }
                Token::Machine(text) => self
                    .pool
                    .number(Number::F64(text.parse().unwrap()))
                    .unwrap(),
                Token::Symbol("(") => {
                    let inner = self.lambda();
                    self.expect(")");
                    inner
                }
                Token::Name(name) if name == "sqrt" => {
                    let arguments = self.arguments();
                    self.apply(Operator::Sqrt, &arguments)
                }
                Token::Name(name) if name == "sum" => self.sum_binder(),
                Token::Name(name) => self.name(&name),
                Token::Symbol(other) => panic!("unexpected {other}"),
            }
        }

        fn sum_binder(&mut self) -> ExprId {
            let start = self.position;
            let mut depth = 0;
            let mut commas = Vec::new();
            let mut position = start;
            loop {
                match &self.tokens[position] {
                    Token::Symbol("(") => depth += 1,
                    Token::Symbol(")") => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Token::Symbol(",") if depth == 1 => commas.push(position),
                    _ => {}
                }
                position += 1;
            }
            let Token::Name(index) = self.tokens[commas[0] + 1].clone() else {
                panic!("expected an index name")
            };
            self.position = start + 1;
            self.scope.push(index.clone());
            let body = self.lambda();
            self.scope.pop();
            self.position = commas[1] + 1;
            let lower = self.lambda();
            self.expect(",");
            let upper = self.lambda();
            self.expect(")");
            let binder = self
                .pool
                .bind(
                    BinderKind::Sum(ReductionShape::LeftFold),
                    &[lower, upper],
                    body,
                )
                .unwrap();
            self.pool.record_bound_name(binder, &index).unwrap();
            binder
        }

        fn name(&mut self, name: &str) -> ExprId {
            if let Some(from_innermost) = self.scope.iter().rev().position(|bound| bound == name) {
                return self
                    .pool
                    .bound(u32::try_from(from_innermost).unwrap())
                    .unwrap();
            }
            if name == "pi" {
                return self.pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
            }
            let symbol = self.pool.intern_symbol(name, SymbolKind::Variable).unwrap();
            self.pool.symbol(symbol).unwrap()
        }
    }

    fn parse_expression(pool: &mut ExprPool, text: &str) -> Result<ExprId, ()> {
        let mut parser = TestParser {
            pool,
            tokens: tokens(text),
            position: 0,
            scope: Vec::new(),
        };
        let expression = parser.lambda();
        assert_eq!(
            parser.position,
            parser.tokens.len(),
            "unparsed rest of {text}"
        );
        Ok(expression)
    }

    fn printed(pool: &ExprPool, expression: ExprId) -> String {
        match pool.node(expression).unwrap() {
            NodeView::Number(number) => match pool.number_value(number).unwrap() {
                Number::Integer(integer) => integer.to_i64().unwrap().to_string(),
                other => format!("{other:?}"),
            },
            NodeView::Symbol(symbol) => pool.symbol_name(symbol).unwrap().to_owned(),
            NodeView::Quantity { value, unit } => {
                let factor = pool.units().factors(unit).unwrap()[0];
                let symbol = pool.units().symbol(factor.named_unit()).unwrap();
                format!("{} {symbol}", printed(pool, value))
            }
            other => format!("{other:?}"),
        }
    }

    struct Fixture {
        pool: ExprPool,
        patterns: Vec<RecognitionPattern>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                pool: ExprPool::new(),
                patterns: Vec::new(),
            }
        }

        fn pattern(mut self, concept: &str, function: &str, conditions: &[&str]) -> Self {
            let function_id = parse_expression(&mut self.pool, function).unwrap();
            let parameters = function.matches("|->").count();
            let conditions = conditions
                .iter()
                .map(|condition| parse_expression(&mut self.pool, condition).unwrap())
                .collect();
            self.patterns.push(RecognitionPattern {
                concept: concept.to_owned(),
                identifier: format!("{concept}/{}", self.patterns.len()),
                parameters,
                function: function_id,
                conditions,
            });
            self
        }

        fn recognize_with(&mut self, text: &str, limits: RecognitionLimits) -> PatternRecognition {
            let root = parse_expression(&mut self.pool, text).unwrap();
            recognize(&mut self.pool, root, &self.patterns, limits, &printed).unwrap()
        }

        fn recognize(&mut self, text: &str) -> PatternRecognition {
            self.recognize_with(text, RecognitionLimits::default())
        }

        fn bindings(&self, found: &RecognizedMatch) -> Vec<String> {
            found
                .bindings
                .iter()
                .map(|binding| printed(&self.pool, *binding))
                .collect()
        }
    }

    fn pythagoras() -> Fixture {
        Fixture::new().pattern(
            "pythagorean-theorem",
            "a |-> b |-> sqrt(a^2 + b^2)",
            &["a |-> b |-> a > 0 and b > 0"],
        )
    }

    fn fraction(numerator: u64, denominator: u64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    #[test]
    fn whole_expression_match_has_coverage_one() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("sqrt(3^2 + 4^2)");

        assert_eq!(found.matches[0].coverage, Number::from(1_i64));
    }

    #[test]
    fn symmetric_bindings_are_listed_once_in_binding_order() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("sqrt(3^2 + 4^2)");

        assert_eq!(
            (found.matches.len(), fixture.bindings(&found.matches[0])),
            (1, vec!["3".to_owned(), "4".to_owned()])
        );
    }

    #[test]
    fn coverage_counts_distinct_nodes_of_the_whole_expression() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("sqrt(3^2 + 4^2) + 1");

        assert_eq!(found.matches[0].coverage, fraction(7, 9));
    }

    #[test]
    fn specificity_counts_distinct_non_variable_pattern_nodes() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("sqrt(3^2 + 4^2)");

        assert_eq!(found.matches[0].specificity, 5);
    }

    #[test]
    fn operands_of_an_addition_match_in_either_order() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("sqrt(4^2 + 3^2)");

        assert_eq!(found.matches.len(), 1);
    }

    #[test]
    fn regrouped_product_chain_matches() {
        let mut fixture = Fixture::new().pattern("volume", "a |-> b |-> c |-> 2 * a * b * c", &[]);

        let found = fixture.recognize("a * (2 * (b * c))");

        assert_eq!(found.matches.len(), 1);
    }

    #[test]
    fn constant_matches_by_identity_and_is_never_evaluated() {
        let mut fixture = Fixture::new().pattern("square", "x |-> x^2 + 1", &[]);

        let found = fixture.recognize("9 + 1");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn decimal_constant_matches_its_interned_integer() {
        let mut fixture = Fixture::new().pattern("square", "x |-> x^2 + 1", &[]);

        let found = fixture.recognize("3^2.0 + 1");

        assert_eq!(found.matches.len(), 1);
    }

    #[test]
    fn machine_constant_does_not_match_an_exact_constant() {
        let mut fixture = Fixture::new().pattern("square", "x |-> x^2 + 1", &[]);

        let found = fixture.recognize("3^f64'2' + 1");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn product_of_equal_factors_is_not_a_power() {
        let mut fixture = Fixture::new().pattern("square", "x |-> x^2 + 1", &[]);

        let found = fixture.recognize("3 * 3 + 1");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn subtraction_keeps_its_argument_order() {
        let mut fixture = Fixture::new().pattern("difference", "x |-> y |-> x^2 - y", &[]);

        let found = fixture.recognize("3 - 4^2");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn left_grouped_chain_offers_its_inner_site() {
        let mut fixture = pythagoras().pattern("sum-of-squares", "a |-> b |-> a^2 + b^2 + 5", &[]);

        let found = fixture.recognize("3^2 + 4^2 + 5");

        assert_eq!(found.matches[0].concept, "sum-of-squares");
    }

    #[test]
    fn chain_of_other_length_does_not_match() {
        let mut fixture = Fixture::new().pattern("two-squares", "a |-> b |-> a^2 * b^2", &[]);

        let found = fixture.recognize("5 * 3^2 * 4^2");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn repeated_variable_needs_equivalent_bindings() {
        let mut fixture = Fixture::new().pattern("double", "x |-> sqrt(x) + sqrt(x)", &[]);

        let equal = fixture.recognize("sqrt(1 + 2) + sqrt(2 + 1)").matches.len();
        let different = fixture.recognize("sqrt(1 + 2) + sqrt(2 + 2)").matches.len();

        assert_eq!((equal, different), (1, 0));
    }

    #[test]
    fn body_of_a_binder_with_its_bound_variable_is_not_a_site() {
        let mut fixture = Fixture::new().pattern("square", "x |-> x^2", &[]);

        let found = fixture.recognize("sum(i^2, i, 1, 4)");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn binding_that_holds_the_index_of_a_matched_binder_is_not_made() {
        let mut fixture = Fixture::new().pattern("three-terms", "f |-> sum(f, i, 1, 3)", &[]);

        let found = fixture.recognize("sum(i^2, i, 1, 3)");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn variable_binds_a_quantity() {
        let mut fixture =
            Fixture::new().pattern("circumference", "r |-> 2 * pi * r", &["r |-> r > 0"]);

        let found = fixture.recognize("2 * pi * 3 cm");

        assert_eq!(fixture.bindings(&found.matches[0]), ["3 cm"]);
    }

    #[test]
    fn pattern_quantity_matches_only_its_own_unit() {
        let mut fixture = Fixture::new().pattern("third-angle", "x |-> y |-> 180 deg - x - y", &[]);

        let degrees = fixture.recognize("180 deg - 50 deg - 60 deg").matches.len();
        let radians = fixture.recognize("180 rad - 50 deg - 60 deg").matches.len();

        assert_eq!((degrees, radians), (1, 0));
    }

    #[test]
    fn false_condition_discards_the_match() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("sqrt((-3)^2 + 4^2)");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn undecided_condition_over_letters_keeps_the_match() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("sqrt(p^2 + q^2)");

        assert_eq!(found.matches[0].holds, [None]);
    }

    #[test]
    fn true_condition_is_recorded() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("sqrt(3^2 + 4^2)");

        assert_eq!(found.matches[0].holds, [Some(true)]);
    }

    fn heron() -> Fixture {
        Fixture::new().pattern(
            "herons-formula",
            "s |-> a |-> b |-> c |-> sqrt(s * (s - a) * (s - b) * (s - c))",
            &["s |-> a |-> b |-> c |-> s == (a + b + c) / 2"],
        )
    }

    #[test]
    fn heron_guard_holds_for_the_exact_half_perimeter() {
        let mut fixture = heron();

        let found = fixture.recognize("sqrt(6 * (6 - 3) * (6 - 4) * (6 - 5))");

        assert_eq!(found.matches[0].holds, [Some(true)]);
    }

    #[test]
    fn heron_guard_is_undecided_for_a_machine_rounded_half_perimeter() {
        let mut fixture = heron();

        let found = fixture.recognize(
            "sqrt(f64'6.000000000000001' * (f64'6.000000000000001' - 3) * (f64'6.000000000000001' - 4) * (f64'6.000000000000001' - 5))",
        );

        assert_eq!(found.matches[0].holds, [None]);
    }

    #[test]
    fn heron_guard_is_false_for_an_exact_wrong_half_perimeter() {
        let mut fixture = heron();

        let found = fixture.recognize("sqrt(7 * (7 - 3) * (7 - 4) * (7 - 5))");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn match_covering_less_than_half_is_not_offered() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("sqrt(3^2 + 4^2) * (5 + 6 + 7 + 8 + 9 + 10 + 11)");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn single_operator_covering_part_of_the_line_is_not_offered() {
        let mut fixture = Fixture::new().pattern("negation", "x |-> -x", &[]);

        let found = fixture.recognize("-(3 + 4) + 1");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn single_operator_covering_the_whole_line_is_offered() {
        let mut fixture = Fixture::new().pattern("negation", "x |-> -x", &[]);

        let found = fixture.recognize("-(3 + 4)");

        assert_eq!(found.matches.len(), 1);
    }

    #[test]
    fn single_operator_with_an_undecided_condition_is_not_offered() {
        let mut fixture = Fixture::new().pattern("negation", "x |-> -x", &["x |-> x >= 0"]);

        let found = fixture.recognize("-y");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn single_operator_reads_a_longer_sum_as_written() {
        let mut fixture = Fixture::new().pattern("sum", "a |-> b |-> a + b", &[]);

        let found = fixture.recognize("1 + 2 + 3");

        assert_eq!(found.matches.len(), 1);
    }

    #[test]
    fn single_operator_reads_a_right_grouped_product_as_written() {
        let mut fixture = Fixture::new().pattern("product", "a |-> b |-> a * b", &[]);

        let found = fixture.recognize("2 * (3 * 4)");

        assert_eq!(found.matches.len(), 1);
    }

    #[test]
    fn specific_concept_ranks_before_a_whole_line_single_operator() {
        let mut fixture = pythagoras().pattern("sum", "a |-> b |-> a + b", &[]);

        let found = fixture.recognize("sqrt(3^2 + 4^2) + 1");

        assert_eq!(found.matches[0].concept, "pythagorean-theorem");
    }

    #[test]
    fn single_operator_holds_its_conditions_at_every_node_of_its_operator() {
        let mut fixture =
            Fixture::new().pattern("sum", "a |-> b |-> a + b", &["a |-> b |-> a > 1 and b > 1"]);

        let found = fixture.recognize("2 + 1 + 3");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn single_operator_judges_an_operand_of_another_operator_by_its_value() {
        let mut fixture = Fixture::new().pattern(
            "product",
            "a |-> b |-> a * b",
            &["a |-> b |-> a > 1 and b > 1"],
        );

        let found = fixture.recognize("(1 + 1) * 3");

        assert_eq!(found.matches.len(), 1);
    }

    #[test]
    fn bare_variable_pattern_is_never_offered() {
        let mut fixture = Fixture::new().pattern("anything", "x |-> x", &[]);

        let found = fixture.recognize("3");

        assert!(found.matches.is_empty());
    }

    #[test]
    fn nothing_recognized_gives_an_empty_untruncated_answer() {
        let mut fixture = pythagoras();

        let found = fixture.recognize("1 + 2");

        assert_eq!(
            found,
            PatternRecognition {
                matches: Vec::new(),
                truncated: false,
                truncated_by: None,
            }
        );
    }

    #[test]
    fn larger_coverage_comes_first() {
        let mut fixture = Fixture::new()
            .pattern("a-inner", "a |-> b |-> a^2 + b^2", &[])
            .pattern("z-outer", "a |-> b |-> sqrt(a^2 + b^2)", &[]);

        let found = fixture.recognize("sqrt(3^2 + 4^2)");

        let concepts: Vec<&str> = found
            .matches
            .iter()
            .map(|found| found.concept.as_str())
            .collect();
        assert_eq!(concepts, ["z-outer", "a-inner"]);
    }

    #[test]
    fn equal_coverage_and_specificity_order_by_concept_identifier() {
        let mut fixture = Fixture::new()
            .pattern("b-concept", "a |-> b |-> sqrt(a^2 + b^2)", &[])
            .pattern("a-concept", "a |-> b |-> sqrt(a^2 + b^2)", &[]);

        let found = fixture.recognize("sqrt(3^2 + 4^2)");

        let concepts: Vec<&str> = found
            .matches
            .iter()
            .map(|found| found.concept.as_str())
            .collect();
        assert_eq!(concepts, ["a-concept", "b-concept"]);
    }

    #[test]
    fn all_true_conditions_come_before_undecided_ones() {
        let mut fixture = Fixture::new()
            .pattern(
                "a-undecided",
                "a |-> b |-> sqrt(a^2 + b^2)",
                &["a |-> b |-> a > c"],
            )
            .pattern(
                "b-true",
                "a |-> b |-> sqrt(a^2 + b^2)",
                &["a |-> b |-> a > 0"],
            );

        let found = fixture.recognize("sqrt(3^2 + 4^2)");

        assert_eq!(found.matches[0].concept, "b-true");
    }

    #[test]
    fn cap_lists_the_first_matches_and_reports_the_cap() {
        let mut fixture = Fixture::new()
            .pattern("a-concept", "a |-> b |-> sqrt(a^2 + b^2)", &[])
            .pattern("b-concept", "a |-> b |-> sqrt(a^2 + b^2)", &[]);
        let limits = RecognitionLimits {
            cap: 1,
            work_budget: 100_000,
        };

        let found = fixture.recognize_with("sqrt(3^2 + 4^2)", limits);

        assert_eq!(
            (found.matches.len(), found.truncated_by),
            (1, Some(RecognitionTruncation::Cap))
        );
    }

    #[test]
    fn exhausted_work_budget_is_reported() {
        let mut fixture = pythagoras();
        let limits = RecognitionLimits {
            cap: 5,
            work_budget: 1,
        };

        let found = fixture.recognize_with("sqrt(3^2 + 4^2)", limits);

        assert_eq!(
            (found.truncated, found.truncated_by),
            (true, Some(RecognitionTruncation::WorkBudget))
        );
    }

    #[test]
    fn cap_outside_its_range_is_an_error() {
        let mut fixture = pythagoras();
        let root = parse_expression(&mut fixture.pool, "1").unwrap();
        let limits = RecognitionLimits {
            cap: 0,
            work_budget: 1,
        };

        let result = recognize(&mut fixture.pool, root, &fixture.patterns, limits, &printed);

        assert_eq!(result, Err(RecognitionError::CapOutOfRange { cap: 0 }));
    }

    #[test]
    fn work_budget_outside_its_range_is_an_error() {
        let mut fixture = pythagoras();
        let root = parse_expression(&mut fixture.pool, "1").unwrap();
        let limits = RecognitionLimits {
            cap: 5,
            work_budget: 10_000_001,
        };

        let result = recognize(&mut fixture.pool, root, &fixture.patterns, limits, &printed);

        assert_eq!(
            result,
            Err(RecognitionError::WorkBudgetOutOfRange {
                work_budget: 10_000_001
            })
        );
    }

    #[test]
    fn pattern_without_its_parameter_binders_is_an_error() {
        let mut fixture = Fixture::new();
        let function = parse_expression(&mut fixture.pool, "x |-> x^2").unwrap();
        let patterns = [RecognitionPattern {
            concept: "square".to_owned(),
            identifier: "square/bad".to_owned(),
            parameters: 2,
            function,
            conditions: Vec::new(),
        }];
        let root = parse_expression(&mut fixture.pool, "3^2").unwrap();

        let result = recognize(
            &mut fixture.pool,
            root,
            &patterns,
            RecognitionLimits::default(),
            &printed,
        );

        assert_eq!(
            result,
            Err(RecognitionError::NotAPatternFunction {
                pattern: "square/bad".to_owned()
            })
        );
    }

    #[test]
    fn same_input_gives_the_same_answer_again() {
        let mut fixture = pythagoras().pattern("sum", "a |-> b |-> a^2 + b^2", &[]);

        let first = fixture.recognize("sqrt(3^2 + 4^2)");
        let second = fixture.recognize("sqrt(3^2 + 4^2)");

        assert_eq!(first, second);
    }

    fn shapes(left: &str, right: &str) -> bool {
        let fixture = Fixture::new()
            .pattern("left", left, &[])
            .pattern("right", right, &[]);
        let mut pool = fixture.pool;
        equivalent_pattern_shapes(&mut pool, &fixture.patterns[0], &fixture.patterns[1]).unwrap()
    }

    #[test]
    fn shapes_equal_up_to_renaming_and_operand_order_are_equivalent() {
        assert!(shapes("a |-> b |-> a^2 + b^2", "x |-> y |-> y^2 + x^2"));
    }

    #[test]
    fn renaming_may_swap_variables_of_an_ordered_operator() {
        assert!(shapes("a |-> b |-> a - b", "x |-> y |-> y - x"));
    }

    #[test]
    fn different_operators_are_not_equivalent_shapes() {
        assert!(!shapes("a |-> b |-> a * b", "x |-> y |-> x + y"));
    }

    #[test]
    fn two_variables_are_not_one_repeated_variable() {
        assert!(!shapes("a |-> b |-> a * b", "x |-> y |-> x * x"));
    }

    #[test]
    fn shapes_with_different_parameter_counts_are_not_equivalent() {
        assert!(!shapes("a |-> a^2", "x |-> y |-> x^2"));
    }
}
