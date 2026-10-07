use std::collections::{BTreeMap, BTreeSet, HashMap};

use calc_exec::{Domain, LowerInput, LowerSpec, ValueForm, lower};
use calc_expr::{BinderKind, ExprId, ExprPool, NodeView, SymbolKind};

const LARGEST_WAY_CAP: u32 = 1000;
const LARGEST_WORK_BUDGET: u64 = 10_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub identifier: String,
    pub output: String,
    pub inputs: Vec<String>,
    pub formula: ExprId,
    pub conditions: Vec<ExprId>,
    pub exact: bool,
    pub sources: Vec<String>,
    pub corpus_references: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleSet {
    pub version: u64,
    pub quantities: Vec<String>,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRequest {
    pub wanted: String,
    pub given: Vec<String>,
    pub obtainable: Vec<String>,
    pub not_obtainable: Vec<String>,
    pub only_obtainable: bool,
    pub way_cap: u32,
    pub work_budget: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Exactness {
    Exact,
    Machine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorBound {
    Zero,
    Documented,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truncation {
    WayCap,
    WorkBudget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WayCondition {
    pub rule: String,
    pub condition: ExprId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Way {
    pub inputs: Vec<String>,
    pub steps: u32,
    pub derivation: Vec<String>,
    pub last_rule: Option<String>,
    pub obtainable: bool,
    pub exactness: Exactness,
    pub error_bound: ErrorBound,
    pub gate_count: Option<u64>,
    pub conditions: Vec<WayCondition>,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivationExpression {
    pub expression: ExprId,
    pub leaves: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WayList {
    pub ways: Vec<Way>,
    pub truncated_by: Option<Truncation>,
    pub complete_through_steps: u32,
    pub ways_found: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchOutcome {
    Reached(WayList),
    NotReached {
        missing_any_of: Vec<Vec<String>>,
        truncated_by: Option<Truncation>,
        complete_through_steps: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchError {
    UnknownQuantity(String),
    DuplicateQuantity(String),
    DuplicateRule(String),
    UnknownRule(String),
    RuleWithUnknownQuantity { rule: String, quantity: String },
    FormulaArityMismatch(String),
    ContradictoryMarks(String),
    WayCapOutOfRange(u32),
    ListCapOutOfRange(u32),
    WorkBudgetOutOfRange(u64),
}

struct Graph {
    names: Vec<String>,
    index_of: HashMap<String, usize>,
    rules: Vec<IndexedRule>,
    rules_by_output: Vec<Vec<usize>>,
}

struct IndexedRule {
    output: usize,
    premises: Vec<usize>,
}

type QuantitySet = BTreeSet<usize>;

impl Graph {
    fn new(rule_set: &RuleSet) -> Result<Self, SearchError> {
        let mut names = rule_set.quantities.clone();
        names.sort();
        if let Some(pair) = names.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(SearchError::DuplicateQuantity(pair[0].clone()));
        }
        let index_of: HashMap<String, usize> = names
            .iter()
            .enumerate()
            .map(|(index, name)| (name.clone(), index))
            .collect();
        let mut identifiers: Vec<&str> = rule_set
            .rules
            .iter()
            .map(|rule| rule.identifier.as_str())
            .collect();
        identifiers.sort_unstable();
        if let Some(pair) = identifiers.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(SearchError::DuplicateRule(pair[0].to_string()));
        }
        let lookup = |rule: &Rule, quantity: &String| {
            index_of
                .get(quantity)
                .copied()
                .ok_or_else(|| SearchError::RuleWithUnknownQuantity {
                    rule: rule.identifier.clone(),
                    quantity: quantity.clone(),
                })
        };
        let mut ordered: Vec<&Rule> = rule_set.rules.iter().collect();
        ordered.sort_by(|left, right| left.identifier.cmp(&right.identifier));
        let mut rules = Vec::new();
        let mut rules_by_output = vec![Vec::new(); names.len()];
        for rule in ordered {
            let output = lookup(rule, &rule.output)?;
            let mut premises = rule
                .inputs
                .iter()
                .map(|input| lookup(rule, input))
                .collect::<Result<Vec<usize>, SearchError>>()?;
            premises.sort_unstable();
            premises.dedup();
            rules_by_output[output].push(rules.len());
            rules.push(IndexedRule { output, premises });
        }
        Ok(Self {
            names,
            index_of,
            rules,
            rules_by_output,
        })
    }

    fn index(&self, name: &str) -> Result<usize, SearchError> {
        self.index_of
            .get(name)
            .copied()
            .ok_or_else(|| SearchError::UnknownQuantity(name.to_string()))
    }

    fn indices(&self, names: &[String]) -> Result<QuantitySet, SearchError> {
        names.iter().map(|name| self.index(name)).collect()
    }

    fn rounds(&self, start: &QuantitySet) -> (Vec<Option<u32>>, Vec<Option<usize>>) {
        let mut round: Vec<Option<u32>> = vec![None; self.names.len()];
        let mut derived_by: Vec<Option<usize>> = vec![None; self.names.len()];
        for quantity in start {
            round[*quantity] = Some(0);
        }
        self.continue_rounds(&mut round, &mut derived_by, 1);
        (round, derived_by)
    }

    fn continue_rounds(
        &self,
        round: &mut [Option<u32>],
        derived_by: &mut [Option<usize>],
        first_round: u32,
    ) -> u32 {
        let mut users: Vec<Vec<usize>> = vec![Vec::new(); self.names.len()];
        let mut missing: Vec<usize> = Vec::with_capacity(self.rules.len());
        let mut ready: BTreeSet<usize> = BTreeSet::new();
        for (rule_index, rule) in self.rules.iter().enumerate() {
            let unknown = rule
                .premises
                .iter()
                .filter(|premise| round[**premise].is_none())
                .count();
            for premise in &rule.premises {
                if round[*premise].is_none() {
                    users[*premise].push(rule_index);
                }
            }
            if unknown == 0 {
                ready.insert(rule_index);
            }
            missing.push(unknown);
        }
        let mut current_round = first_round;
        let mut last_round = first_round.saturating_sub(1);
        loop {
            let mut next = Vec::new();
            for rule_index in std::mem::take(&mut ready) {
                let output = self.rules[rule_index].output;
                if round[output].is_none() {
                    round[output] = Some(current_round);
                    derived_by[output] = Some(rule_index);
                    next.push(output);
                }
            }
            if next.is_empty() {
                return last_round;
            }
            last_round = current_round;
            for quantity in next {
                for rule_index in &users[quantity] {
                    missing[*rule_index] -= 1;
                    if missing[*rule_index] == 0 {
                        ready.insert(*rule_index);
                    }
                }
            }
            current_round += 1;
        }
    }

    fn staged(&self, stages: &[QuantitySet]) -> Staged {
        let mut round: Vec<Option<u32>> = vec![None; self.names.len()];
        let mut derived_by: Vec<Option<usize>> = vec![None; self.names.len()];
        let mut leaves = QuantitySet::new();
        let mut known_after = Vec::new();
        let mut next_round = 1;
        for stage in stages {
            for quantity in stage {
                if round[*quantity].is_none() {
                    round[*quantity] = Some(0);
                    leaves.insert(*quantity);
                }
            }
            let last = self.continue_rounds(&mut round, &mut derived_by, next_round);
            next_round = next_round.max(last + 1);
            known_after.push(
                (0..self.names.len())
                    .filter(|quantity| round[*quantity].is_some())
                    .collect(),
            );
        }
        Staged {
            round,
            derived_by,
            leaves,
            known_after,
        }
    }

    fn reaches(&self, start: &QuantitySet, wanted: usize) -> bool {
        self.rounds(start).0[wanted].is_some()
    }
}

struct Staged {
    round: Vec<Option<u32>>,
    derived_by: Vec<Option<usize>>,
    leaves: QuantitySet,
    known_after: Vec<QuantitySet>,
}

impl Staged {
    fn derivation(&self, graph: &Graph, ordered_rules: &[&Rule], quantity: usize) -> Vec<usize> {
        let mut needed: BTreeSet<usize> = BTreeSet::new();
        let mut pending = vec![quantity];
        while let Some(current) = pending.pop() {
            if self.leaves.contains(&current) {
                continue;
            }
            if let Some(deriving) = self.derived_by[current]
                && needed.insert(deriving)
            {
                pending.extend(graph.rules[deriving].premises.iter().copied());
            }
        }
        let mut derivation: Vec<usize> = needed.into_iter().collect();
        derivation.sort_by_key(|index| {
            (
                self.round[graph.rules[*index].output],
                ordered_rules[*index].identifier.clone(),
            )
        });
        derivation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedDerivation {
    pub rule: String,
    pub derivation: Vec<String>,
    pub steps: u32,
}

pub fn staged_derivation(
    rule_set: &RuleSet,
    wanted: &str,
    stages: &[Vec<String>],
) -> Result<Option<StagedDerivation>, SearchError> {
    let graph = Graph::new(rule_set)?;
    let wanted = graph.index(wanted)?;
    let stages = stages
        .iter()
        .map(|stage| graph.indices(stage))
        .collect::<Result<Vec<QuantitySet>, SearchError>>()?;
    let mut ordered_rules: Vec<&Rule> = rule_set.rules.iter().collect();
    ordered_rules.sort_by(|left, right| left.identifier.cmp(&right.identifier));
    let staged = graph.staged(&stages);
    if staged.leaves.contains(&wanted) {
        return Ok(None);
    }
    let Some(rule_index) = staged.derived_by[wanted] else {
        return Ok(None);
    };
    Ok(Some(StagedDerivation {
        rule: ordered_rules[rule_index].identifier.clone(),
        derivation: staged
            .derivation(&graph, &ordered_rules, wanted)
            .iter()
            .map(|index| ordered_rules[*index].identifier.clone())
            .collect(),
        steps: staged.round[wanted].unwrap_or(0),
    }))
}

struct Enumeration<'graph> {
    graph: &'graph Graph,
    wanted: usize,
    free: QuantitySet,
    allowed: QuantitySet,
    work_left: u64,
    depth_was_cut: bool,
    budget_exhausted: bool,
}

impl Enumeration<'_> {
    fn spend(&mut self) -> bool {
        if self.work_left == 0 {
            self.budget_exhausted = true;
            return false;
        }
        self.work_left -= 1;
        true
    }

    fn sufficient(&self, inputs: &QuantitySet) -> bool {
        let start: QuantitySet = inputs.union(&self.free).copied().collect();
        self.graph.reaches(&start, self.wanted)
    }

    fn expand(&mut self, quantity: usize, path: &mut Vec<usize>, depth: u32) -> Vec<QuantitySet> {
        let mut results = Vec::new();
        if depth == 0 {
            if !self.graph.rules_by_output[quantity].is_empty() {
                self.depth_was_cut = true;
            }
            return results;
        }
        for rule_index in self.graph.rules_by_output[quantity].clone() {
            let premises = self.graph.rules[rule_index].premises.clone();
            if premises.iter().any(|premise| path.contains(premise)) {
                continue;
            }
            let mut partial: Vec<QuantitySet> = vec![QuantitySet::new()];
            for premise in premises {
                let mut options: Vec<QuantitySet> = Vec::new();
                if self.free.contains(&premise) {
                    options.push(QuantitySet::new());
                } else {
                    if self.allowed.contains(&premise) {
                        options.push(QuantitySet::from([premise]));
                    }
                    path.push(premise);
                    options.extend(self.expand(premise, path, depth - 1));
                    path.pop();
                }
                let mut combined = Vec::new();
                for prefix in &partial {
                    for option in &options {
                        if !self.spend() {
                            return results;
                        }
                        combined.push(prefix.union(option).copied().collect());
                    }
                }
                partial = combined;
                if partial.is_empty() {
                    break;
                }
            }
            results.extend(partial);
        }
        results
    }

    fn reduce(&self, candidate: &QuantitySet) -> QuantitySet {
        let mut current = candidate.clone();
        for element in candidate {
            current.remove(element);
            if !self.sufficient(&current) {
                current.insert(*element);
            }
        }
        current
    }
}

struct Enumerated {
    ways: Vec<QuantitySet>,
    truncated_by: Option<Truncation>,
    complete_through_steps: u32,
}

fn enumerate(
    graph: &Graph,
    wanted: usize,
    free: QuantitySet,
    allowed: QuantitySet,
    way_cap: u32,
    work_budget: u64,
) -> Enumerated {
    let mut enumeration = Enumeration {
        graph,
        wanted,
        free,
        allowed,
        work_left: work_budget,
        depth_was_cut: false,
        budget_exhausted: false,
    };
    let cap = usize::try_from(way_cap).unwrap_or(usize::MAX);
    if enumeration.sufficient(&QuantitySet::new()) {
        return Enumerated {
            ways: vec![QuantitySet::new()],
            truncated_by: None,
            complete_through_steps: 0,
        };
    }
    let mut ways: Vec<QuantitySet> = Vec::new();
    let mut complete_through_steps = 0;
    let largest_depth = u32::try_from(graph.names.len()).unwrap_or(u32::MAX);
    for depth in 1..=largest_depth {
        enumeration.depth_was_cut = false;
        let mut path = vec![wanted];
        let candidates = enumeration.expand(wanted, &mut path, depth);
        for candidate in candidates {
            let way = enumeration.reduce(&candidate);
            if !ways.contains(&way) {
                ways.push(way);
                if ways.len() >= cap {
                    return Enumerated {
                        ways,
                        truncated_by: Some(Truncation::WayCap),
                        complete_through_steps,
                    };
                }
            }
        }
        if enumeration.budget_exhausted {
            return Enumerated {
                ways,
                truncated_by: Some(Truncation::WorkBudget),
                complete_through_steps,
            };
        }
        complete_through_steps = depth;
        if !enumeration.depth_was_cut {
            break;
        }
    }
    Enumerated {
        ways,
        truncated_by: None,
        complete_through_steps,
    }
}

pub(crate) fn instantiate(
    pool: &mut ExprPool,
    formula: ExprId,
    arguments: &[ExprId],
) -> Option<ExprId> {
    let mut body = formula;
    for _ in arguments {
        match pool.node(body).ok()? {
            NodeView::Bind {
                binder: BinderKind::Lambda,
                body: inner,
                ..
            } => body = inner,
            _ => return None,
        }
    }
    if matches!(
        pool.node(body).ok()?,
        NodeView::Bind {
            binder: BinderKind::Lambda,
            ..
        }
    ) {
        return None;
    }
    replace_parameters(pool, body, arguments, 0)
}

pub(crate) fn replace_parameters(
    pool: &mut ExprPool,
    expression: ExprId,
    arguments: &[ExprId],
    depth: u32,
) -> Option<ExprId> {
    let count = u32::try_from(arguments.len()).ok()?;
    match pool.node(expression).ok()? {
        NodeView::Bound(index) if index >= depth => {
            let parameter = index - depth;
            if parameter < count {
                let position = usize::try_from(count - 1 - parameter).ok()?;
                arguments.get(position).copied()
            } else {
                pool.bound(index - count).ok()
            }
        }
        NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_) => Some(expression),
        NodeView::Apply {
            head,
            arguments: children,
        } => {
            let children = children.to_vec();
            let replaced = children
                .iter()
                .map(|child| replace_parameters(pool, *child, arguments, depth))
                .collect::<Option<Vec<ExprId>>>()?;
            pool.apply(head, &replaced).ok()
        }
        NodeView::Bind {
            binder,
            arguments: children,
            body,
        } => {
            let children = children.to_vec();
            let replaced = children
                .iter()
                .map(|child| replace_parameters(pool, *child, arguments, depth))
                .collect::<Option<Vec<ExprId>>>()?;
            let body = replace_parameters(pool, body, arguments, depth + 1)?;
            pool.bind(binder, &replaced, body).ok()
        }
        NodeView::Quantity { value, unit } => {
            let value = replace_parameters(pool, value, arguments, depth)?;
            pool.quantity(value, unit).ok()
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let replaced = elements
                .iter()
                .map(|element| replace_parameters(pool, *element, arguments, depth))
                .collect::<Option<Vec<ExprId>>>()?;
            pool.array(&shape, &replaced).ok()
        }
    }
}

struct Scorer<'data> {
    graph: &'data Graph,
    ordered_rules: Vec<&'data Rule>,
    wanted: usize,
    free: QuantitySet,
    obtainable: QuantitySet,
}

impl Scorer<'_> {
    fn way(&self, pool: &mut ExprPool, inputs: &QuantitySet) -> Result<Way, SearchError> {
        let start: QuantitySet = inputs.union(&self.free).copied().collect();
        let (round, derived_by) = self.graph.rounds(&start);
        let mut needed: BTreeSet<usize> = BTreeSet::new();
        let mut pending = vec![self.wanted];
        while let Some(quantity) = pending.pop() {
            if start.contains(&quantity) {
                continue;
            }
            if let Some(rule_index) = derived_by[quantity]
                && needed.insert(rule_index)
            {
                pending.extend(self.graph.rules[rule_index].premises.iter().copied());
            }
        }
        let mut derivation: Vec<usize> = needed.into_iter().collect();
        derivation.sort_by_key(|rule_index| {
            let output = self.graph.rules[*rule_index].output;
            (
                round[output],
                self.ordered_rules[*rule_index].identifier.clone(),
            )
        });
        let derivation_rules: Vec<&Rule> = derivation
            .iter()
            .map(|rule_index| self.ordered_rules[*rule_index])
            .collect();
        let exactness = if derivation_rules.iter().all(|rule| rule.exact) {
            Exactness::Exact
        } else {
            Exactness::Machine
        };
        let gate_count = self.gates(pool, &start, &derivation)?;
        let error_bound = match (exactness, gate_count) {
            (Exactness::Exact, _) => ErrorBound::Zero,
            (Exactness::Machine, Some(_)) => ErrorBound::Documented,
            (Exactness::Machine, None) => ErrorBound::Unknown,
        };
        let is_known =
            |quantity: &usize| self.free.contains(quantity) || self.obtainable.contains(quantity);
        Ok(Way {
            inputs: inputs
                .iter()
                .map(|quantity| self.graph.names[*quantity].clone())
                .collect(),
            steps: round[self.wanted].unwrap_or(0),
            last_rule: derived_by[self.wanted]
                .filter(|_| !start.contains(&self.wanted))
                .map(|rule_index| self.ordered_rules[rule_index].identifier.clone()),
            derivation: derivation_rules
                .iter()
                .map(|rule| rule.identifier.clone())
                .collect(),
            obtainable: inputs.iter().all(is_known),
            exactness,
            error_bound,
            gate_count,
            sources: derivation_rules
                .iter()
                .flat_map(|rule| rule.sources.iter().cloned())
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect(),
            conditions: derivation_rules
                .iter()
                .flat_map(|rule| {
                    rule.conditions.iter().map(|condition| WayCondition {
                        rule: rule.identifier.clone(),
                        condition: *condition,
                    })
                })
                .collect(),
        })
    }

    fn gates(
        &self,
        pool: &mut ExprPool,
        start: &QuantitySet,
        derivation: &[usize],
    ) -> Result<Option<u64>, SearchError> {
        let mut expressions: BTreeMap<usize, ExprId> = BTreeMap::new();
        let mut inputs = Vec::new();
        for quantity in start {
            let Ok(symbol) = pool.intern_symbol(&self.graph.names[*quantity], SymbolKind::Variable)
            else {
                return Ok(None);
            };
            let Ok(node) = pool.symbol(symbol) else {
                return Ok(None);
            };
            expressions.insert(*quantity, node);
            inputs.push(LowerInput {
                symbol,
                form: ValueForm::Real,
            });
        }
        for rule_index in derivation {
            let indexed = &self.graph.rules[*rule_index];
            let rule = self.ordered_rules[*rule_index];
            let Some(arguments) = rule
                .inputs
                .iter()
                .map(|input| {
                    self.graph
                        .index_of
                        .get(input)
                        .and_then(|quantity| expressions.get(quantity))
                        .copied()
                })
                .collect::<Option<Vec<ExprId>>>()
            else {
                return Ok(None);
            };
            let instance = instantiate(pool, rule.formula, &arguments)
                .ok_or_else(|| SearchError::FormulaArityMismatch(rule.identifier.clone()))?;
            expressions.insert(indexed.output, instance);
        }
        let Some(root) = expressions.get(&self.wanted).copied() else {
            return Ok(None);
        };
        let spec = LowerSpec {
            domain: Domain::F64,
            inputs,
            output: ValueForm::Real,
        };
        let Ok(plan) = lower(pool, root, &spec) else {
            return Ok(None);
        };
        Ok(Some(u64::try_from(plan.gate_count()).unwrap_or(u64::MAX)))
    }
}

pub fn derivation_expression(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    wanted: &str,
    derivation: &[String],
) -> Result<DerivationExpression, SearchError> {
    let (expressions, leaves) = derivation_expressions(pool, rule_set, wanted, derivation)?;
    let expression = expressions
        .get(wanted)
        .copied()
        .ok_or_else(|| SearchError::UnknownQuantity(wanted.to_string()))?;
    Ok(DerivationExpression {
        expression,
        leaves: leaves.into_iter().collect(),
    })
}

pub fn derivation_expressions(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    wanted: &str,
    derivation: &[String],
) -> Result<(BTreeMap<String, ExprId>, BTreeSet<String>), SearchError> {
    let rules = derivation
        .iter()
        .map(|identifier| {
            rule_set
                .rules
                .iter()
                .find(|rule| &rule.identifier == identifier)
                .ok_or_else(|| SearchError::UnknownRule(identifier.clone()))
        })
        .collect::<Result<Vec<&Rule>, SearchError>>()?;
    let derived: BTreeSet<&str> = rules.iter().map(|rule| rule.output.as_str()).collect();
    let mut leaves: BTreeSet<String> = rules
        .iter()
        .flat_map(|rule| rule.inputs.iter())
        .filter(|input| !derived.contains(input.as_str()))
        .cloned()
        .collect();
    if rules.is_empty() {
        leaves.insert(wanted.to_string());
    }
    let mut expressions: BTreeMap<String, ExprId> = BTreeMap::new();
    for leaf in &leaves {
        let symbol = pool
            .intern_symbol(leaf, SymbolKind::Variable)
            .map_err(|_| SearchError::UnknownQuantity(leaf.clone()))?;
        let node = pool
            .symbol(symbol)
            .map_err(|_| SearchError::UnknownQuantity(leaf.clone()))?;
        expressions.insert(leaf.clone(), node);
    }
    for rule in rules {
        let arguments = rule
            .inputs
            .iter()
            .map(|input| {
                expressions
                    .get(input)
                    .copied()
                    .ok_or_else(|| SearchError::UnknownQuantity(input.clone()))
            })
            .collect::<Result<Vec<ExprId>, SearchError>>()?;
        let instance = instantiate(pool, rule.formula, &arguments)
            .ok_or_else(|| SearchError::FormulaArityMismatch(rule.identifier.clone()))?;
        expressions.insert(rule.output.clone(), instance);
    }
    Ok((expressions, leaves))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForwardRequest {
    pub given: Vec<String>,
    pub obtainable: Vec<String>,
    pub cap: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForwardEntry {
    pub name: String,
    pub steps: u32,
    pub rule: String,
    pub inputs: Vec<String>,
    pub derivation: Vec<String>,
    pub uses: Vec<String>,
    pub conditions: Vec<WayCondition>,
    pub needs_obtainable: bool,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForwardListing {
    pub entries: Vec<ForwardEntry>,
    pub reachable_total: u32,
    pub truncated: bool,
}

pub fn list_forward(
    rule_set: &RuleSet,
    request: &ForwardRequest,
) -> Result<ForwardListing, SearchError> {
    let graph = Graph::new(rule_set)?;
    if !(1..=LARGEST_WAY_CAP).contains(&request.cap) {
        return Err(SearchError::ListCapOutOfRange(request.cap));
    }
    let given = graph.indices(&request.given)?;
    let obtainable = graph.indices(&request.obtainable)?;
    if let Some(conflict) = given.intersection(&obtainable).next() {
        return Err(SearchError::ContradictoryMarks(
            graph.names[*conflict].clone(),
        ));
    }
    let mut ordered_rules: Vec<&Rule> = rule_set.rules.iter().collect();
    ordered_rules.sort_by(|left, right| left.identifier.cmp(&right.identifier));

    let staged = graph.staged(&[given.clone(), obtainable.clone()]);
    let from_given = staged.known_after[0].clone();
    let round = &staged.round;
    let derived_by = &staged.derived_by;
    let start: QuantitySet = given.union(&obtainable).copied().collect();
    let mut reachable: Vec<usize> = (0..graph.names.len())
        .filter(|quantity| !start.contains(quantity))
        .filter(|quantity| derived_by[*quantity].is_some())
        .collect();
    reachable.sort_by(|left, right| {
        round[*left]
            .cmp(&round[*right])
            .then_with(|| graph.names[*left].cmp(&graph.names[*right]))
    });
    let reachable_total = u32::try_from(reachable.len()).unwrap_or(u32::MAX);
    let cap = usize::try_from(request.cap).unwrap_or(usize::MAX);
    let truncated = reachable.len() > cap;
    let entries = reachable
        .into_iter()
        .take(cap)
        .filter_map(|quantity| {
            let rule_index = derived_by[quantity]?;
            let derivation = staged.derivation(&graph, &ordered_rules, quantity);
            let rules: Vec<&Rule> = derivation
                .iter()
                .map(|index| ordered_rules[*index])
                .collect();
            let uses: BTreeSet<String> = derivation
                .iter()
                .flat_map(|index| graph.rules[*index].premises.iter())
                .filter(|premise| staged.leaves.contains(premise))
                .map(|premise| graph.names[*premise].clone())
                .collect();
            let mut inputs = ordered_rules[rule_index].inputs.clone();
            inputs.sort();
            inputs.dedup();
            Some(ForwardEntry {
                name: graph.names[quantity].clone(),
                steps: round[quantity].unwrap_or(0),
                rule: ordered_rules[rule_index].identifier.clone(),
                inputs,
                derivation: rules.iter().map(|rule| rule.identifier.clone()).collect(),
                uses: uses.into_iter().collect(),
                conditions: rules
                    .iter()
                    .flat_map(|rule| {
                        rule.conditions.iter().map(|condition| WayCondition {
                            rule: rule.identifier.clone(),
                            condition: *condition,
                        })
                    })
                    .collect(),
                needs_obtainable: !from_given.contains(&quantity),
                sources: rules
                    .iter()
                    .flat_map(|rule| rule.sources.iter().cloned())
                    .collect::<BTreeSet<String>>()
                    .into_iter()
                    .collect(),
            })
        })
        .collect();
    Ok(ForwardListing {
        entries,
        reachable_total,
        truncated,
    })
}

fn check_request(graph: &Graph, request: &SearchRequest) -> Result<(), SearchError> {
    if !(1..=LARGEST_WAY_CAP).contains(&request.way_cap) {
        return Err(SearchError::WayCapOutOfRange(request.way_cap));
    }
    if !(1..=LARGEST_WORK_BUDGET).contains(&request.work_budget) {
        return Err(SearchError::WorkBudgetOutOfRange(request.work_budget));
    }
    graph.index(&request.wanted)?;
    for name in request
        .given
        .iter()
        .chain(&request.obtainable)
        .chain(&request.not_obtainable)
    {
        graph.index(name)?;
    }
    if let Some(conflict) = request
        .not_obtainable
        .iter()
        .find(|name| request.given.contains(name) || request.obtainable.contains(name))
    {
        return Err(SearchError::ContradictoryMarks(conflict.clone()));
    }
    Ok(())
}

pub fn search_ways(
    pool: &mut ExprPool,
    rule_set: &RuleSet,
    request: &SearchRequest,
) -> Result<SearchOutcome, SearchError> {
    let graph = Graph::new(rule_set)?;
    check_request(&graph, request)?;
    let wanted = graph.index(&request.wanted)?;
    let free = graph.indices(&request.given)?;
    let marked_obtainable = graph.indices(&request.obtainable)?;
    let not_obtainable = graph.indices(&request.not_obtainable)?;
    let mut ordered_rules: Vec<&Rule> = rule_set.rules.iter().collect();
    ordered_rules.sort_by(|left, right| left.identifier.cmp(&right.identifier));
    let every_quantity: QuantitySet = (0..graph.names.len())
        .filter(|quantity| *quantity != wanted && !not_obtainable.contains(quantity))
        .collect();
    let allowed: QuantitySet = if request.only_obtainable {
        marked_obtainable.clone()
    } else {
        every_quantity.clone()
    };

    let enumerated = enumerate(
        &graph,
        wanted,
        free.clone(),
        allowed,
        request.way_cap,
        request.work_budget,
    );
    if enumerated.ways.is_empty() && enumerated.truncated_by.is_none() {
        let known: QuantitySet = free.union(&marked_obtainable).copied().collect();
        let further = enumerate(
            &graph,
            wanted,
            known,
            every_quantity,
            request.way_cap,
            request.work_budget,
        );
        let mut missing: Vec<Vec<String>> = further
            .ways
            .iter()
            .map(|set| {
                set.iter()
                    .map(|quantity| graph.names[*quantity].clone())
                    .collect()
            })
            .collect();
        missing.sort_by(|left, right| left.len().cmp(&right.len()).then_with(|| left.cmp(right)));
        return Ok(SearchOutcome::NotReached {
            missing_any_of: missing,
            truncated_by: further.truncated_by,
            complete_through_steps: further.complete_through_steps,
        });
    }

    let scorer = Scorer {
        graph: &graph,
        ordered_rules,
        wanted,
        free,
        obtainable: marked_obtainable,
    };
    let ways = enumerated
        .ways
        .iter()
        .map(|inputs| scorer.way(pool, inputs))
        .collect::<Result<Vec<Way>, SearchError>>()?;
    let ways_found = u32::try_from(ways.len()).unwrap_or(u32::MAX);
    Ok(SearchOutcome::Reached(WayList {
        ways,
        truncated_by: enumerated.truncated_by,
        complete_through_steps: enumerated.complete_through_steps,
        ways_found,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::{Head, Operator};
    use calc_numbers::Number;

    const AREA: &str = "circle.area";
    const CHORD: &str = "circle.chord";
    const CIRCUMFERENCE: &str = "circle.circumference";
    const DIAMETER: &str = "circle.diameter";
    const RADIUS: &str = "circle.radius";
    const SAGITTA: &str = "circle.sagitta";

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn integer(pool: &mut ExprPool, value: i64) -> ExprId {
        pool.number(Number::from(value)).unwrap()
    }

    fn lambda(pool: &mut ExprPool, parameters: usize, body: ExprId) -> ExprId {
        (0..parameters).fold(body, |inner, _| {
            pool.bind(BinderKind::Lambda, &[], inner).unwrap()
        })
    }

    fn pi(pool: &mut ExprPool) -> ExprId {
        pool.symbol(calc_expr::BuiltinConstant::Pi.symbol())
            .unwrap()
    }

    fn rule(identifier: &str, output: &str, inputs: &[&str], formula: ExprId, exact: bool) -> Rule {
        Rule {
            identifier: identifier.to_string(),
            output: output.to_string(),
            inputs: inputs.iter().map(|input| input.to_string()).collect(),
            formula,
            conditions: Vec::new(),
            exact,
            sources: vec!["openstax-prealgebra-2e".to_string()],
            corpus_references: Vec::new(),
        }
    }

    fn circle(pool: &mut ExprPool) -> RuleSet {
        let x = pool.bound(0).unwrap();
        let outer = pool.bound(1).unwrap();
        let two = integer(pool, 2);
        let eight = integer(pool, 8);
        let pi = pi(pool);
        let squared = apply(pool, Operator::Pow, &[x, two]);
        let pi_squared = apply(pool, Operator::Mul, &[pi, squared]);
        let area_from_radius = lambda(pool, 1, pi_squared);
        let over_pi = apply(pool, Operator::Div, &[x, pi]);
        let root = apply(pool, Operator::Sqrt, &[over_pi]);
        let radius_from_area = lambda(pool, 1, root);
        let two_pi = apply(pool, Operator::Mul, &[two, pi]);
        let two_pi_x = apply(pool, Operator::Mul, &[two_pi, x]);
        let circumference_from_radius = lambda(pool, 1, two_pi_x);
        let x_over_two_pi = apply(pool, Operator::Div, &[x, two_pi]);
        let radius_from_circumference = lambda(pool, 1, x_over_two_pi);
        let pi_x = apply(pool, Operator::Mul, &[pi, x]);
        let circumference_from_diameter = lambda(pool, 1, pi_x);
        let diameter_from_circumference = lambda(pool, 1, over_pi);
        let two_x = apply(pool, Operator::Mul, &[two, x]);
        let diameter_from_radius = lambda(pool, 1, two_x);
        let half = apply(pool, Operator::Div, &[x, two]);
        let radius_from_diameter = lambda(pool, 1, half);
        let chord_squared = apply(pool, Operator::Pow, &[outer, two]);
        let eight_h = apply(pool, Operator::Mul, &[eight, x]);
        let fraction = apply(pool, Operator::Div, &[chord_squared, eight_h]);
        let sum = apply(pool, Operator::Add, &[half, fraction]);
        let radius_from_chord_and_sagitta = lambda(pool, 2, sum);
        let positive = apply(pool, Operator::Greater, &[x, two]);
        let mut from_radius = rule(
            "circle-area/from-radius",
            AREA,
            &[RADIUS],
            area_from_radius,
            false,
        );
        from_radius.conditions.push(positive);
        RuleSet {
            version: 0x1234,
            quantities: [AREA, CHORD, CIRCUMFERENCE, DIAMETER, RADIUS, SAGITTA]
                .iter()
                .map(|name| name.to_string())
                .collect(),
            rules: vec![
                from_radius,
                rule(
                    "circle-area/radius-from-area",
                    RADIUS,
                    &[AREA],
                    radius_from_area,
                    false,
                ),
                rule(
                    "circle-circumference/from-radius",
                    CIRCUMFERENCE,
                    &[RADIUS],
                    circumference_from_radius,
                    false,
                ),
                rule(
                    "circle-circumference/radius-from-circumference",
                    RADIUS,
                    &[CIRCUMFERENCE],
                    radius_from_circumference,
                    false,
                ),
                rule(
                    "circle-circumference/from-diameter",
                    CIRCUMFERENCE,
                    &[DIAMETER],
                    circumference_from_diameter,
                    false,
                ),
                rule(
                    "circle-circumference/diameter-from-circumference",
                    DIAMETER,
                    &[CIRCUMFERENCE],
                    diameter_from_circumference,
                    false,
                ),
                rule(
                    "circle-diameter/from-radius",
                    DIAMETER,
                    &[RADIUS],
                    diameter_from_radius,
                    true,
                ),
                rule(
                    "circle-diameter/radius-from-diameter",
                    RADIUS,
                    &[DIAMETER],
                    radius_from_diameter,
                    true,
                ),
                rule(
                    "circle-sagitta/radius-from-chord-and-sagitta",
                    RADIUS,
                    &[CHORD, SAGITTA],
                    radius_from_chord_and_sagitta,
                    true,
                ),
            ],
        }
    }

    fn request(wanted: &str) -> SearchRequest {
        SearchRequest {
            wanted: wanted.to_string(),
            given: Vec::new(),
            obtainable: Vec::new(),
            not_obtainable: Vec::new(),
            only_obtainable: false,
            way_cap: 1000,
            work_budget: 1_000_000,
        }
    }

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    fn reached(outcome: SearchOutcome) -> WayList {
        match outcome {
            SearchOutcome::Reached(list) => list,
            other => panic!("expected ways, found {other:?}"),
        }
    }

    fn run(request: &SearchRequest) -> WayList {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);
        reached(search_ways(&mut pool, &rules, request).unwrap())
    }

    fn input_sets(list: &WayList) -> BTreeSet<Vec<String>> {
        list.ways.iter().map(|way| way.inputs.clone()).collect()
    }

    fn way_with<'list>(list: &'list WayList, inputs: &[&str]) -> &'list Way {
        list.ways
            .iter()
            .find(|way| way.inputs == names(inputs))
            .unwrap()
    }

    #[test]
    fn ways_to_the_circumference_are_the_minimal_input_sets() {
        let list = run(&request(CIRCUMFERENCE));

        assert_eq!(
            input_sets(&list),
            BTreeSet::from([
                names(&[AREA]),
                names(&[CHORD, SAGITTA]),
                names(&[DIAMETER]),
                names(&[RADIUS]),
            ])
        );
    }

    #[test]
    fn complete_enumeration_is_not_truncated() {
        let list = run(&request(CIRCUMFERENCE));

        assert_eq!((list.truncated_by, list.ways_found), (None, 4));
    }

    #[test]
    fn steps_are_the_generation_rounds() {
        let list = run(&request(CIRCUMFERENCE));

        assert_eq!(
            (
                way_with(&list, &[RADIUS]).steps,
                way_with(&list, &[CHORD, SAGITTA]).steps
            ),
            (1, 2)
        );
    }

    #[test]
    fn canonical_derivation_lists_rules_by_round() {
        let list = run(&request(CIRCUMFERENCE));

        let way = way_with(&list, &[CHORD, SAGITTA]);
        assert_eq!(
            (way.derivation.clone(), way.last_rule.clone()),
            (
                names(&[
                    "circle-sagitta/radius-from-chord-and-sagitta",
                    "circle-circumference/from-radius"
                ]),
                Some("circle-circumference/from-radius".to_string())
            )
        );
    }

    #[test]
    fn canonical_derivation_takes_the_rule_known_earliest() {
        let list = run(&request(CIRCUMFERENCE));

        assert_eq!(
            way_with(&list, &[DIAMETER]).derivation,
            names(&["circle-circumference/from-diameter"])
        );
    }

    #[test]
    fn ways_are_found_in_order_of_rounds() {
        let list = run(&request(CIRCUMFERENCE));

        let steps: Vec<u32> = list.ways.iter().map(|way| way.steps).collect();
        assert_eq!(steps.last(), Some(&2));
    }

    #[test]
    fn not_obtainable_quantity_is_never_an_input() {
        let mut blocked = request(CIRCUMFERENCE);
        blocked.not_obtainable = names(&[RADIUS]);

        let list = run(&blocked);

        assert_eq!(
            input_sets(&list),
            BTreeSet::from([names(&[AREA]), names(&[CHORD, SAGITTA]), names(&[DIAMETER])])
        );
    }

    #[test]
    fn only_obtainable_lists_ways_over_marked_quantities() {
        let mut restricted = request(CIRCUMFERENCE);
        restricted.obtainable = names(&[DIAMETER]);
        restricted.only_obtainable = true;

        let list = run(&restricted);

        assert_eq!(input_sets(&list), BTreeSet::from([names(&[DIAMETER])]));
    }

    #[test]
    fn way_is_obtainable_when_all_inputs_are_marked() {
        let mut marked = request(CIRCUMFERENCE);
        marked.obtainable = names(&[CHORD, SAGITTA]);

        let list = run(&marked);

        assert_eq!(
            (
                way_with(&list, &[CHORD, SAGITTA]).obtainable,
                way_with(&list, &[RADIUS]).obtainable
            ),
            (true, false)
        );
    }

    #[test]
    fn given_quantity_that_reaches_the_wanted_gives_the_empty_way() {
        let mut given = request(AREA);
        given.given = names(&[DIAMETER]);

        let list = run(&given);

        assert_eq!(
            (input_sets(&list), list.ways[0].steps),
            (BTreeSet::from([Vec::new()]), 2)
        );
    }

    #[test]
    fn given_quantity_is_not_an_input_of_the_remaining_ways() {
        let mut given = request(CIRCUMFERENCE);
        given.given = names(&[CHORD]);

        let list = run(&given);

        assert!(input_sets(&list).contains(&names(&[SAGITTA])));
    }

    #[test]
    fn wanted_quantity_in_the_given_takes_no_steps() {
        let mut given = request(RADIUS);
        given.given = names(&[RADIUS]);

        let list = run(&given);

        assert_eq!(
            (
                list.ways[0].inputs.len(),
                list.ways[0].steps,
                list.ways[0].last_rule.clone()
            ),
            (0, 0, None)
        );
    }

    #[test]
    fn unreached_wanted_lists_the_further_inputs_one_element_sets_first() {
        let mut partial = request(CIRCUMFERENCE);
        partial.given = names(&[CHORD]);
        partial.only_obtainable = true;
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);

        let outcome = search_ways(&mut pool, &rules, &partial).unwrap();

        assert_eq!(
            outcome,
            SearchOutcome::NotReached {
                missing_any_of: vec![
                    names(&[AREA]),
                    names(&[DIAMETER]),
                    names(&[RADIUS]),
                    names(&[SAGITTA])
                ],
                truncated_by: None,
                complete_through_steps: 4,
            }
        );
    }

    #[test]
    fn wanted_that_no_rule_yields_has_no_further_inputs() {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);

        let outcome = search_ways(&mut pool, &rules, &request(CHORD)).unwrap();

        assert!(matches!(
            outcome,
            SearchOutcome::NotReached { missing_any_of, truncated_by: None, .. } if missing_any_of.is_empty()
        ));
    }

    #[test]
    fn way_cap_truncates_and_keeps_the_completed_rounds() {
        let mut capped = request(CIRCUMFERENCE);
        capped.way_cap = 3;

        let list = run(&capped);

        assert_eq!(
            (
                list.ways.len(),
                list.truncated_by,
                list.complete_through_steps
            ),
            (3, Some(Truncation::WayCap), 1)
        );
    }

    #[test]
    fn every_way_with_steps_through_the_completed_round_is_listed_when_truncated() {
        let mut capped = request(CIRCUMFERENCE);
        capped.way_cap = 3;

        let list = run(&capped);

        assert_eq!(
            input_sets(&list),
            BTreeSet::from([names(&[AREA]), names(&[DIAMETER]), names(&[RADIUS])])
        );
    }

    #[test]
    fn work_budget_truncates() {
        let mut limited = request(CIRCUMFERENCE);
        limited.work_budget = 1;

        let list = run(&limited);

        assert_eq!(list.truncated_by, Some(Truncation::WorkBudget));
    }

    #[test]
    fn independent_alternatives_multiply_the_ways() {
        let mut pool = ExprPool::new();
        let x = pool.bound(0).unwrap();
        let identity = lambda(&mut pool, 1, x);
        let wanted_formula = lambda(&mut pool, 3, x);
        let mut quantities = vec!["w".to_string()];
        let mut rules = Vec::new();
        for index in 0..3 {
            for prefix in ["a", "b", "x"] {
                quantities.push(format!("{prefix}{index}"));
            }
            for prefix in ["a", "b"] {
                rules.push(rule(
                    &format!("x{index}/from-{prefix}"),
                    &format!("x{index}"),
                    &[&format!("{prefix}{index}")],
                    identity,
                    true,
                ));
            }
        }
        rules.push(rule(
            "w/from-x",
            "w",
            &["x0", "x1", "x2"],
            wanted_formula,
            true,
        ));
        let rule_set = RuleSet {
            version: 1,
            quantities,
            rules,
        };

        let list = reached(search_ways(&mut pool, &rule_set, &request("w")).unwrap());

        assert_eq!(list.ways.len(), 27);
    }

    #[test]
    fn exact_rules_give_exactness_exact_and_zero_error_bound() {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);

        let list = reached(search_ways(&mut pool, &rules, &request(DIAMETER)).unwrap());

        let way = way_with(&list, &[RADIUS]);
        assert_eq!(
            (way.exactness, way.error_bound),
            (Exactness::Exact, ErrorBound::Zero)
        );
    }

    #[test]
    fn machine_rule_gives_a_documented_error_bound_and_a_gate_count() {
        let list = run(&request(CIRCUMFERENCE));

        let way = way_with(&list, &[RADIUS]);
        assert_eq!(
            (way.exactness, way.error_bound, way.gate_count.is_some()),
            (Exactness::Machine, ErrorBound::Documented, true)
        );
    }

    #[test]
    fn longer_derivation_has_more_gates() {
        let list = run(&request(CIRCUMFERENCE));

        assert!(
            way_with(&list, &[CHORD, SAGITTA]).gate_count.unwrap()
                > way_with(&list, &[RADIUS]).gate_count.unwrap()
        );
    }

    #[test]
    fn way_carries_the_conditions_of_its_rules() {
        let list = run(&request(AREA));

        let way = way_with(&list, &[RADIUS]);
        assert_eq!(
            way.conditions
                .iter()
                .map(|condition| condition.rule.clone())
                .collect::<Vec<_>>(),
            names(&["circle-area/from-radius"])
        );
    }

    #[test]
    fn answer_does_not_depend_on_the_order_of_rules_and_quantities() {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);
        let mut shuffled = rules.clone();
        shuffled.rules.reverse();
        shuffled.quantities.reverse();

        let first = search_ways(&mut pool, &rules, &request(CIRCUMFERENCE)).unwrap();
        let second = search_ways(&mut pool, &shuffled, &request(CIRCUMFERENCE)).unwrap();

        assert_eq!(first, second);
    }

    fn brute_force_ways(
        rule_set: &RuleSet,
        wanted: &str,
        given: &[String],
    ) -> BTreeSet<Vec<String>> {
        let graph = Graph::new(rule_set).unwrap();
        let wanted = graph.index(wanted).unwrap();
        let free = graph.indices(given).unwrap();
        let candidates: Vec<usize> = (0..graph.names.len())
            .filter(|quantity| *quantity != wanted && !free.contains(quantity))
            .collect();
        let sufficient: Vec<QuantitySet> = (0..1_usize << candidates.len())
            .map(|mask| {
                candidates
                    .iter()
                    .enumerate()
                    .filter(|(bit, _)| mask & (1 << bit) != 0)
                    .map(|(_, quantity)| *quantity)
                    .collect::<QuantitySet>()
            })
            .filter(|set| {
                let start: QuantitySet = set.union(&free).copied().collect();
                graph.reaches(&start, wanted)
            })
            .collect();
        sufficient
            .iter()
            .filter(|set| {
                !sufficient
                    .iter()
                    .any(|other| other != *set && other.is_subset(set))
            })
            .map(|set| {
                set.iter()
                    .map(|quantity| graph.names[*quantity].clone())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn ways_equal_the_brute_force_minimal_sets_on_random_rule_systems() {
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        let mut pool = ExprPool::new();
        let body = integer(&mut pool, 1);
        let formulas: Vec<ExprId> = (0..=3)
            .map(|arity| lambda(&mut pool, arity, body))
            .collect();
        let mut mismatches = Vec::new();
        for trial in 0..300 {
            let size = 2 + next(5);
            let quantities: Vec<String> = (0..size).map(|index| format!("q{index}")).collect();
            let mut rules = Vec::new();
            for index in 0..1 + next(8) {
                let arity = next(size.min(3) + 1);
                let mut inputs: Vec<String> = Vec::new();
                while u64::try_from(inputs.len()).unwrap() < arity {
                    let candidate = quantities[usize::try_from(next(size)).unwrap()].clone();
                    if !inputs.contains(&candidate) {
                        inputs.push(candidate);
                    }
                }
                rules.push(Rule {
                    identifier: format!("rule/{index}"),
                    output: quantities[usize::try_from(next(size)).unwrap()].clone(),
                    formula: formulas[inputs.len()],
                    inputs,
                    conditions: Vec::new(),
                    exact: true,
                    sources: Vec::new(),
                    corpus_references: Vec::new(),
                });
            }
            let rule_set = RuleSet {
                version: 0,
                quantities: quantities.clone(),
                rules,
            };
            let wanted = quantities[usize::try_from(next(size)).unwrap()].clone();
            let given: Vec<String> = quantities
                .iter()
                .filter(|quantity| **quantity != wanted && next(4) == 0)
                .cloned()
                .collect();
            let mut search = request(&wanted);
            search.given = given.clone();
            let expected = brute_force_ways(&rule_set, &wanted, &given);
            let found = match search_ways(&mut pool, &rule_set, &search).unwrap() {
                SearchOutcome::Reached(list) => input_sets(&list),
                SearchOutcome::NotReached { .. } => BTreeSet::new(),
            };
            if found != expected {
                mismatches.push(trial);
            }
        }

        assert!(mismatches.is_empty(), "{mismatches:?}");
    }

    #[test]
    fn way_carries_the_sorted_sources_of_its_derivation() {
        let mut pool = ExprPool::new();
        let mut rules = circle(&mut pool);
        rules.rules[8].sources = names(&["zeta-source", "openstax-prealgebra-2e"]);

        let list = reached(search_ways(&mut pool, &rules, &request(CIRCUMFERENCE)).unwrap());

        assert_eq!(
            way_with(&list, &[CHORD, SAGITTA]).sources,
            names(&["openstax-prealgebra-2e", "zeta-source"])
        );
    }

    #[test]
    fn derivation_expression_substitutes_the_formulas_over_the_leaves() {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);
        let derivation = names(&[
            "circle-diameter/radius-from-diameter",
            "circle-area/from-radius",
        ]);

        let built = derivation_expression(&mut pool, &rules, AREA, &derivation).unwrap();

        let symbol = pool.lookup_symbol(DIAMETER).unwrap();
        let leaf = pool.symbol(symbol).unwrap();
        let two = integer(&mut pool, 2);
        let pi = pi(&mut pool);
        let half = apply(&mut pool, Operator::Div, &[leaf, two]);
        let squared = apply(&mut pool, Operator::Pow, &[half, two]);
        let expected = apply(&mut pool, Operator::Mul, &[pi, squared]);
        assert_eq!(
            (built.expression, built.leaves),
            (expected, names(&[DIAMETER]))
        );
    }

    #[test]
    fn derivation_with_an_unknown_rule_is_an_error() {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);

        assert_eq!(
            derivation_expression(&mut pool, &rules, AREA, &names(&["circle-area/guess"])),
            Err(SearchError::UnknownRule("circle-area/guess".to_string()))
        );
    }

    fn forward(given: &[&str], obtainable: &[&str], cap: u32) -> ForwardRequest {
        ForwardRequest {
            given: names(given),
            obtainable: names(obtainable),
            cap,
        }
    }

    fn worked_example(pool: &mut ExprPool) -> RuleSet {
        let x = pool.bound(0).unwrap();
        let single = lambda(pool, 1, x);
        let pair = lambda(pool, 2, x);
        RuleSet {
            version: 36,
            quantities: names(&["r", "p", "q", "x", "y"]),
            rules: vec![
                rule("p-from-r", "p", &["r"], single, true),
                rule("q-from-p", "q", &["p"], single, true),
                rule("q-from-x", "q", &["x"], single, true),
                rule("y-from-q-x", "y", &["q", "x"], pair, true),
            ],
        }
    }

    fn entry_summary(
        entry: &ForwardEntry,
    ) -> (
        String,
        u32,
        String,
        Vec<String>,
        Vec<String>,
        Vec<String>,
        bool,
    ) {
        (
            entry.name.clone(),
            entry.steps,
            entry.rule.clone(),
            entry.inputs.clone(),
            entry.derivation.clone(),
            entry.uses.clone(),
            entry.needs_obtainable,
        )
    }

    #[test]
    fn forward_listing_reproduces_the_worked_example_of_a_036() {
        let mut pool = ExprPool::new();
        let rules = worked_example(&mut pool);

        let listing = list_forward(&rules, &forward(&["r"], &["x"], 20)).unwrap();

        let summaries: Vec<_> = listing.entries.iter().map(entry_summary).collect();
        assert_eq!(
            (summaries, listing.reachable_total, listing.truncated),
            (
                vec![
                    (
                        "p".to_string(),
                        1,
                        "p-from-r".to_string(),
                        names(&["r"]),
                        names(&["p-from-r"]),
                        names(&["r"]),
                        false
                    ),
                    (
                        "q".to_string(),
                        2,
                        "q-from-p".to_string(),
                        names(&["p"]),
                        names(&["p-from-r", "q-from-p"]),
                        names(&["r"]),
                        false
                    ),
                    (
                        "y".to_string(),
                        3,
                        "y-from-q-x".to_string(),
                        names(&["q", "x"]),
                        names(&["p-from-r", "q-from-p", "y-from-q-x"]),
                        names(&["r", "x"]),
                        true
                    ),
                ],
                3,
                false
            )
        );
    }

    #[test]
    fn entries_from_the_given_alone_match_the_way_search_for_them() {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);
        let listing = list_forward(&rules, &forward(&[DIAMETER], &[], 20)).unwrap();

        let mismatches: Vec<String> = listing
            .entries
            .iter()
            .filter(|entry| {
                let mut search = request(&entry.name);
                search.given = names(&[DIAMETER]);
                let list = reached(search_ways(&mut pool, &rules, &search).unwrap());
                let way = &list.ways[0];
                !(way.inputs.is_empty()
                    && way.steps == entry.steps
                    && way.derivation == entry.derivation)
            })
            .map(|entry| entry.name.clone())
            .collect();

        assert_eq!(
            (mismatches, listing.reachable_total),
            (Vec::<String>::new(), 3)
        );
    }

    #[test]
    fn entry_that_needs_an_obtainable_role_is_reached_only_with_it() {
        let mut pool = ExprPool::new();
        let rules = worked_example(&mut pool);
        let mut with_given = request("y");
        with_given.given = names(&["r"]);
        with_given.only_obtainable = true;
        let mut with_obtainable = with_given.clone();
        with_obtainable.given = names(&["r", "x"]);

        let without = search_ways(&mut pool, &rules, &with_given).unwrap();
        let with = search_ways(&mut pool, &rules, &with_obtainable).unwrap();

        assert!(matches!(without, SearchOutcome::NotReached { .. }));
        assert_eq!(input_sets(&reached(with)), BTreeSet::from([Vec::new()]));
    }

    #[test]
    fn obtainable_role_already_following_from_the_given_keeps_its_own_derivation() {
        let mut pool = ExprPool::new();
        let rules = worked_example(&mut pool);

        let listing = list_forward(&rules, &forward(&["r"], &["q", "x"], 20)).unwrap();

        let y = listing
            .entries
            .iter()
            .find(|entry| entry.name == "y")
            .unwrap();
        assert_eq!(
            (y.derivation.clone(), y.uses.clone()),
            (
                names(&["p-from-r", "q-from-p", "y-from-q-x"]),
                names(&["r", "x"])
            )
        );
    }

    #[test]
    fn given_and_obtainable_roles_are_never_listed() {
        let mut pool = ExprPool::new();
        let rules = worked_example(&mut pool);

        let listing = list_forward(&rules, &forward(&["r"], &["x"], 20)).unwrap();

        assert!(
            listing
                .entries
                .iter()
                .all(|entry| entry.name != "r" && entry.name != "x")
        );
    }

    #[test]
    fn quantity_of_a_rule_without_inputs_is_listed_with_one_step() {
        let mut pool = ExprPool::new();
        let mut rules = worked_example(&mut pool);
        let constant = integer(&mut pool, 7);
        rules
            .rules
            .push(rule("r-constant", "r", &[], constant, true));

        let listing = list_forward(&rules, &forward(&[], &[], 20)).unwrap();

        let summaries: Vec<(String, u32)> = listing
            .entries
            .iter()
            .map(|entry| (entry.name.clone(), entry.steps))
            .collect();
        assert_eq!(
            summaries,
            vec![
                ("r".to_string(), 1),
                ("p".to_string(), 2),
                ("q".to_string(), 3)
            ]
        );
    }

    #[test]
    fn entries_of_one_step_count_are_ordered_by_name() {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);

        let listing = list_forward(&rules, &forward(&[RADIUS], &[], 20)).unwrap();

        let order: Vec<(u32, String)> = listing
            .entries
            .iter()
            .map(|entry| (entry.steps, entry.name.clone()))
            .collect();
        assert_eq!(
            order,
            vec![
                (1, AREA.to_string()),
                (1, CIRCUMFERENCE.to_string()),
                (1, DIAMETER.to_string())
            ]
        );
    }

    #[test]
    fn cap_truncates_the_listing_and_keeps_the_exact_total() {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);

        let listing = list_forward(&rules, &forward(&[RADIUS], &[], 2)).unwrap();

        assert_eq!(
            (
                listing.entries.len(),
                listing.reachable_total,
                listing.truncated
            ),
            (2, 3, true)
        );
    }

    #[test]
    fn nothing_reachable_gives_an_empty_listing() {
        let mut pool = ExprPool::new();
        let rules = circle(&mut pool);

        let listing = list_forward(&rules, &forward(&[CHORD], &[], 20)).unwrap();

        assert_eq!(
            (
                listing.entries.len(),
                listing.reachable_total,
                listing.truncated
            ),
            (0, 0, false)
        );
    }

    #[test]
    fn entry_carries_the_sources_and_conditions_of_its_whole_derivation() {
        let mut pool = ExprPool::new();
        let mut rules = circle(&mut pool);
        let diameter_rule = rules
            .rules
            .iter_mut()
            .find(|rule| rule.identifier == "circle-diameter/radius-from-diameter")
            .unwrap();
        diameter_rule.sources = names(&["zeta-source"]);

        let listing = list_forward(&rules, &forward(&[DIAMETER], &[], 20)).unwrap();

        let area = listing
            .entries
            .iter()
            .find(|entry| entry.name == AREA)
            .unwrap();
        assert_eq!(
            (
                area.sources.clone(),
                area.conditions
                    .iter()
                    .map(|condition| condition.rule.clone())
                    .collect::<Vec<_>>()
            ),
            (
                names(&["openstax-prealgebra-2e", "zeta-source"]),
                names(&["circle-area/from-radius"])
            )
        );
    }

    #[test]
    fn forward_listing_does_not_depend_on_the_order_of_rules() {
        let mut pool = ExprPool::new();
        let rules = worked_example(&mut pool);
        let mut reversed = rules.clone();
        reversed.rules.reverse();
        reversed.quantities.reverse();

        assert_eq!(
            list_forward(&rules, &forward(&["r"], &["x"], 20)),
            list_forward(&reversed, &forward(&["r"], &["x"], 20))
        );
    }

    #[test]
    fn unknown_given_role_in_a_forward_request_is_an_error() {
        let mut pool = ExprPool::new();
        let rules = worked_example(&mut pool);

        assert_eq!(
            list_forward(&rules, &forward(&["z"], &[], 20)),
            Err(SearchError::UnknownQuantity("z".to_string()))
        );
    }

    #[test]
    fn role_both_given_and_obtainable_is_contradictory() {
        let mut pool = ExprPool::new();
        let rules = worked_example(&mut pool);

        assert_eq!(
            list_forward(&rules, &forward(&["r"], &["r"], 20)),
            Err(SearchError::ContradictoryMarks("r".to_string()))
        );
    }

    #[test]
    fn list_cap_outside_its_range_is_an_error() {
        let mut pool = ExprPool::new();
        let rules = worked_example(&mut pool);

        assert_eq!(
            list_forward(&rules, &forward(&["r"], &[], 0)),
            Err(SearchError::ListCapOutOfRange(0))
        );
    }

    fn error(request: &SearchRequest, adjust: impl FnOnce(&mut RuleSet)) -> SearchError {
        let mut pool = ExprPool::new();
        let mut rules = circle(&mut pool);
        adjust(&mut rules);
        search_ways(&mut pool, &rules, request).unwrap_err()
    }

    #[test]
    fn unknown_wanted_quantity_is_an_error() {
        assert_eq!(
            error(&request("circle.colour"), |_| {}),
            SearchError::UnknownQuantity("circle.colour".to_string())
        );
    }

    #[test]
    fn rule_with_unknown_quantity_is_an_error() {
        let found = error(&request(AREA), |rules| {
            rules.rules[0].inputs = names(&["circle.colour"])
        });

        assert_eq!(
            found,
            SearchError::RuleWithUnknownQuantity {
                rule: "circle-area/from-radius".to_string(),
                quantity: "circle.colour".to_string()
            }
        );
    }

    #[test]
    fn duplicate_rule_identifier_is_an_error() {
        let found = error(&request(AREA), |rules| {
            let copy = rules.rules[0].clone();
            rules.rules.push(copy);
        });

        assert_eq!(
            found,
            SearchError::DuplicateRule("circle-area/from-radius".to_string())
        );
    }

    #[test]
    fn duplicate_quantity_is_an_error() {
        let found = error(&request(AREA), |rules| {
            rules.quantities.push(AREA.to_string())
        });

        assert_eq!(found, SearchError::DuplicateQuantity(AREA.to_string()));
    }

    #[test]
    fn formula_with_too_few_parameters_is_an_error() {
        let found = error(&request(CIRCUMFERENCE), |rules| {
            let single = rules.rules[1].formula;
            let sagitta_rule = rules.rules.len() - 1;
            rules.rules[sagitta_rule].formula = single;
        });

        assert_eq!(
            found,
            SearchError::FormulaArityMismatch(
                "circle-sagitta/radius-from-chord-and-sagitta".to_string()
            )
        );
    }

    #[test]
    fn quantity_marked_obtainable_and_not_obtainable_is_contradictory() {
        let mut marks = request(AREA);
        marks.obtainable = names(&[RADIUS]);
        marks.not_obtainable = names(&[RADIUS]);

        assert_eq!(
            error(&marks, |_| {}),
            SearchError::ContradictoryMarks(RADIUS.to_string())
        );
    }

    #[test]
    fn way_cap_outside_its_range_is_an_error() {
        let mut capped = request(AREA);
        capped.way_cap = 0;

        assert_eq!(error(&capped, |_| {}), SearchError::WayCapOutOfRange(0));
    }

    #[test]
    fn work_budget_outside_its_range_is_an_error() {
        let mut limited = request(AREA);
        limited.work_budget = LARGEST_WORK_BUDGET + 1;

        assert_eq!(
            error(&limited, |_| {}),
            SearchError::WorkBudgetOutOfRange(LARGEST_WORK_BUDGET + 1)
        );
    }
}
