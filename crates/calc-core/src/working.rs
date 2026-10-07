use std::collections::{HashMap, VecDeque};

use calc_exec::{Backend, Domain, Preference};
use calc_expr::{AccessError, ExprId, ExprPool, Head, NodeView, Operator, SymbolId};
use calc_numbers::Number;

use crate::computed_result::ResultValue;
use crate::exact_evaluation::{ExactEvaluationError, evaluate_exact};
use crate::machine_evaluation::{MachineEvaluationError, evaluate_f32, evaluate_f64};

pub const STEP_LIMIT: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepValue {
    Number(Number),
    Expression(ExprId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOperation {
    Operator(Operator),
    Reference(SymbolId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    Exact,
    Machine(Domain),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepOperand {
    pub value: StepValue,
    pub path: Option<Vec<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingStep {
    pub path: Vec<usize>,
    pub operation: StepOperation,
    pub operands: Vec<StepOperand>,
    pub result: StepValue,
    pub kind: StepKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Working {
    pub steps: Vec<WorkingStep>,
    pub further_steps: usize,
    pub further_paths: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkingError {
    Access(AccessError),
    UnknownPath,
    NoOperation,
    ApproximateOperations,
    Exact(ExactEvaluationError),
    Machine(MachineEvaluationError),
}

#[derive(Clone, Debug, Default)]
pub struct References {
    values: HashMap<SymbolId, Number>,
}

impl References {
    pub fn new() -> References {
        References {
            values: HashMap::new(),
        }
    }

    pub fn with(mut self, symbol: SymbolId, value: Number) -> References {
        self.values.insert(symbol, value);
        self
    }

    fn value_of(&self, symbol: SymbolId) -> Option<&Number> {
        self.values.get(&symbol)
    }
}

fn operands_of(view: &NodeView<'_>) -> Option<Vec<ExprId>> {
    match view {
        NodeView::Apply {
            head: Head::Operator(_),
            arguments,
        } => Some(arguments.to_vec()),
        _ => None,
    }
}

fn operator_of(view: &NodeView<'_>) -> Option<Operator> {
    match view {
        NodeView::Apply {
            head: Head::Operator(operator),
            ..
        } => Some(*operator),
        _ => None,
    }
}

pub fn node_at(pool: &ExprPool, root: ExprId, path: &[usize]) -> Result<ExprId, WorkingError> {
    let mut node = root;
    for index in path {
        let view = pool.node(node).map_err(WorkingError::Access)?;
        let operands = operands_of(&view).ok_or(WorkingError::UnknownPath)?;
        node = *operands.get(*index).ok_or(WorkingError::UnknownPath)?;
    }
    Ok(node)
}

struct Plan {
    expanded: Vec<(ExprId, Vec<usize>)>,
    further: Vec<Vec<usize>>,
    further_count: usize,
}

fn is_step(pool: &ExprPool, node: ExprId, references: &References) -> Result<bool, WorkingError> {
    let view = pool.node(node).map_err(WorkingError::Access)?;
    Ok(match &view {
        NodeView::Symbol(symbol) => references.value_of(*symbol).is_some(),
        other => operator_of(other).is_some(),
    })
}

fn count_steps(
    pool: &ExprPool,
    node: ExprId,
    references: &References,
) -> Result<usize, WorkingError> {
    let mut count = 0;
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if !is_step(pool, current, references)? {
            continue;
        }
        count += 1;
        let view = pool.node(current).map_err(WorkingError::Access)?;
        if let Some(operands) = operands_of(&view) {
            pending.extend(operands);
        }
    }
    Ok(count)
}

fn plan_steps(
    pool: &ExprPool,
    root: ExprId,
    start_path: &[usize],
    references: &References,
    limit: usize,
) -> Result<Plan, WorkingError> {
    let mut expanded = Vec::new();
    let mut further = Vec::new();
    let mut further_count = 0;
    let mut pending: VecDeque<(ExprId, Vec<usize>)> = VecDeque::new();
    pending.push_back((root, start_path.to_vec()));
    while let Some((node, path)) = pending.pop_front() {
        if !is_step(pool, node, references)? {
            continue;
        }
        let view = pool.node(node).map_err(WorkingError::Access)?;
        if expanded.len() >= limit {
            further_count += count_steps(pool, node, references)?;
            if !further.contains(&path) {
                further.push(path);
            }
            continue;
        }
        expanded.push((node, path.clone()));
        if let Some(operands) = operands_of(&view) {
            for (index, operand) in operands.into_iter().enumerate() {
                let mut operand_path = path.clone();
                operand_path.push(index);
                pending.push_back((operand, operand_path));
            }
        }
    }
    Ok(Plan {
        expanded,
        further,
        further_count,
    })
}

fn substituted_root(
    pool: &mut ExprPool,
    root: ExprId,
    references: &References,
) -> Result<ExprId, WorkingError> {
    if references.values.is_empty() {
        return Ok(root);
    }
    let mut replacements = HashMap::new();
    for (symbol, value) in &references.values {
        let node = pool
            .number(value.clone())
            .map_err(|_| WorkingError::NoOperation)?;
        replacements.insert(*symbol, node);
    }
    calc_expr::substitute_symbols(pool, root, &replacements).map_err(|_| WorkingError::NoOperation)
}

trait Evaluator {
    fn kind(&self) -> StepKind;
    fn value(&mut self, pool: &mut ExprPool, node: ExprId) -> Result<StepValue, WorkingError>;
}

struct ExactEvaluator;

impl Evaluator for ExactEvaluator {
    fn kind(&self) -> StepKind {
        StepKind::Exact
    }

    fn value(&mut self, pool: &mut ExprPool, node: ExprId) -> Result<StepValue, WorkingError> {
        let evaluation = evaluate_exact(pool, node).map_err(WorkingError::Exact)?;
        Ok(match evaluation.rational_value() {
            Some(number) => StepValue::Number(number.clone()),
            None => StepValue::Expression(evaluation.expression()),
        })
    }
}

struct MachineEvaluator<'backends> {
    domain: Domain,
    backends: &'backends [&'backends dyn Backend],
    preference: Preference,
}

impl Evaluator for MachineEvaluator<'_> {
    fn kind(&self) -> StepKind {
        StepKind::Machine(self.domain)
    }

    fn value(&mut self, pool: &mut ExprPool, node: ExprId) -> Result<StepValue, WorkingError> {
        let evaluation = match self.domain {
            Domain::F64 => evaluate_f64(pool, node, self.backends, self.preference),
            Domain::F32 => evaluate_f32(pool, node, self.backends, self.preference),
        }
        .map_err(WorkingError::Machine)?;
        if evaluation.has_approximate_operations() {
            return Err(WorkingError::ApproximateOperations);
        }
        match evaluation.result().value() {
            ResultValue::Number(number) => Ok(StepValue::Number(number.clone())),
            _ => Err(WorkingError::NoOperation),
        }
    }
}

fn working(
    pool: &mut ExprPool,
    root: ExprId,
    path: &[usize],
    references: &References,
    limit: usize,
    evaluator: &mut dyn Evaluator,
) -> Result<Working, WorkingError> {
    let written = node_at(pool, root, path)?;
    let start = substituted_root(pool, written, references)?;
    let plan = plan_steps(pool, written, path, references, limit)?;
    let mut steps = Vec::new();
    for (node, node_path) in plan.expanded.iter().rev() {
        let view = pool.node(*node).map_err(WorkingError::Access)?;
        if let NodeView::Symbol(symbol) = view {
            let value = references
                .value_of(symbol)
                .ok_or(WorkingError::NoOperation)?
                .clone();
            steps.push(WorkingStep {
                path: node_path.clone(),
                operation: StepOperation::Reference(symbol),
                operands: Vec::new(),
                result: StepValue::Number(value),
                kind: evaluator.kind(),
            });
            continue;
        }
        let operator = operator_of(&view).ok_or(WorkingError::NoOperation)?;
        let operand_nodes = operands_of(&view).ok_or(WorkingError::NoOperation)?;
        let mut operands = Vec::new();
        for (index, operand) in operand_nodes.iter().enumerate() {
            let mut operand_path = node_path.clone();
            operand_path.push(index);
            let operand_root = node_at(pool, start, &operand_path[path.len()..])?;
            let value = evaluator.value(pool, operand_root)?;
            let operand_view = pool.node(*operand).map_err(WorkingError::Access)?;
            let has_own_steps = operator_of(&operand_view).is_some()
                || matches!(operand_view, NodeView::Symbol(symbol) if references.value_of(symbol).is_some());
            operands.push(StepOperand {
                value,
                path: has_own_steps.then_some(operand_path),
            });
        }
        let result_root = node_at(pool, start, &node_path[path.len()..])?;
        let result = evaluator.value(pool, result_root)?;
        steps.push(WorkingStep {
            path: node_path.clone(),
            operation: StepOperation::Operator(operator),
            operands,
            result,
            kind: evaluator.kind(),
        });
    }
    if steps.is_empty() {
        return Err(WorkingError::NoOperation);
    }
    Ok(Working {
        steps,
        further_steps: plan.further_count,
        further_paths: plan.further,
    })
}

pub fn exact_working(
    pool: &mut ExprPool,
    root: ExprId,
    path: &[usize],
    references: &References,
) -> Result<Working, WorkingError> {
    working(
        pool,
        root,
        path,
        references,
        STEP_LIMIT,
        &mut ExactEvaluator,
    )
}

pub fn machine_working(
    pool: &mut ExprPool,
    root: ExprId,
    path: &[usize],
    references: &References,
    domain: Domain,
    backends: &[&dyn Backend],
    preference: Preference,
) -> Result<Working, WorkingError> {
    working(
        pool,
        root,
        path,
        references,
        STEP_LIMIT,
        &mut MachineEvaluator {
            domain,
            backends,
            preference,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_exec_cpu::CpuBackend;
    use calc_expr::SymbolKind;
    use calc_syntax::parse_expression;

    fn parsed(pool: &mut ExprPool, text: &str) -> ExprId {
        parse_expression(pool, text).expect("parses")
    }

    fn integer(value: i64) -> StepValue {
        StepValue::Number(Number::from(value))
    }

    fn exact(pool: &mut ExprPool, text: &str) -> Working {
        let root = parsed(pool, text);
        exact_working(pool, root, &[], &References::new()).expect("working")
    }

    #[test]
    fn a_sum_inside_a_product_is_worked_from_the_inside_out() {
        let mut pool = ExprPool::new();

        let working = exact(&mut pool, "(2 + 3) * 4");

        let shown: Vec<(&[usize], StepOperation, StepValue)> = working
            .steps
            .iter()
            .map(|step| (step.path.as_slice(), step.operation, step.result.clone()))
            .collect();
        assert_eq!(
            shown,
            [
                (
                    &[0usize][..],
                    StepOperation::Operator(Operator::Add),
                    integer(5)
                ),
                (&[][..], StepOperation::Operator(Operator::Mul), integer(20)),
            ]
        );
    }

    #[test]
    fn an_operand_that_is_an_operation_carries_its_own_path() {
        let mut pool = ExprPool::new();

        let working = exact(&mut pool, "(2 + 3) * 4");

        let last = working.steps.last().expect("a step");
        let paths: Vec<Option<&[usize]>> = last
            .operands
            .iter()
            .map(|operand| operand.path.as_deref())
            .collect();
        assert_eq!(paths, [Some(&[0usize][..]), None]);
    }

    #[test]
    fn an_operand_carries_the_value_it_had_in_the_step() {
        let mut pool = ExprPool::new();

        let working = exact(&mut pool, "(2 + 3) * 4");

        let last = working.steps.last().expect("a step");
        let values: Vec<StepValue> = last
            .operands
            .iter()
            .map(|operand| operand.value.clone())
            .collect();
        assert_eq!(values, [integer(5), integer(4)]);
    }

    #[test]
    fn a_line_a_reference_stands_for_is_a_step_of_its_own() {
        let mut pool = ExprPool::new();
        let symbol = pool
            .intern_symbol("r1", SymbolKind::Variable)
            .expect("symbol");
        let root = parsed(&mut pool, "r1 * 2");

        let references = References::new().with(symbol, Number::from(7));
        let working = exact_working(&mut pool, root, &[], &references).expect("working");

        let first = working.steps.first().expect("a step");
        assert_eq!(
            (first.operation, first.result.clone()),
            (StepOperation::Reference(symbol), integer(7))
        );
    }

    #[test]
    fn a_reference_is_the_value_of_the_step_that_uses_it() {
        let mut pool = ExprPool::new();
        let symbol = pool
            .intern_symbol("r1", SymbolKind::Variable)
            .expect("symbol");
        let root = parsed(&mut pool, "r1 * 2");

        let references = References::new().with(symbol, Number::from(7));
        let working = exact_working(&mut pool, root, &[], &references).expect("working");

        let last = working.steps.last().expect("a step");
        assert_eq!(last.result, integer(14));
    }

    #[test]
    fn a_request_for_a_path_works_only_that_subexpression() {
        let mut pool = ExprPool::new();
        let root = parsed(&mut pool, "(2 + 3) * 4");

        let working = exact_working(&mut pool, root, &[0], &References::new()).expect("working");

        let paths: Vec<&[usize]> = working
            .steps
            .iter()
            .map(|step| step.path.as_slice())
            .collect();
        assert_eq!(paths, [&[0usize][..]]);
    }

    #[test]
    fn a_path_that_names_no_operand_is_refused() {
        let mut pool = ExprPool::new();
        let root = parsed(&mut pool, "2 + 3");

        let working = exact_working(&mut pool, root, &[5], &References::new());

        assert_eq!(working, Err(WorkingError::UnknownPath));
    }

    #[test]
    fn a_line_without_an_operation_has_no_working() {
        let mut pool = ExprPool::new();
        let root = parsed(&mut pool, "42");

        let working = exact_working(&mut pool, root, &[], &References::new());

        assert_eq!(working, Err(WorkingError::NoOperation));
    }

    #[test]
    fn a_working_longer_than_the_limit_names_what_it_did_not_expand() {
        let mut pool = ExprPool::new();
        let text = format!("1 {}", " + 1".repeat(STEP_LIMIT + 20));
        let root = parsed(&mut pool, &text);

        let working = exact_working(&mut pool, root, &[], &References::new()).expect("working");

        assert_eq!(
            (
                working.steps.len(),
                working.further_steps,
                working.further_paths.is_empty()
            ),
            (STEP_LIMIT, 20, false)
        );
    }

    #[test]
    fn every_unexpanded_path_is_one_request_of_its_own() {
        let mut pool = ExprPool::new();
        let text = format!("1 {}", " + 1".repeat(STEP_LIMIT + 20));
        let root = parsed(&mut pool, &text);

        let working = exact_working(&mut pool, root, &[], &References::new()).expect("working");

        let further = working.further_paths.first().expect("a path").clone();
        let opened = exact_working(&mut pool, root, &further, &References::new()).expect("working");
        assert_eq!(
            opened.steps.last().map(|step| step.path.clone()),
            Some(further)
        );
    }

    #[test]
    fn a_machine_working_names_the_format_of_its_steps() {
        let mut pool = ExprPool::new();
        let root = parsed(&mut pool, "0.1 + 0.2");
        let backend = CpuBackend::new();

        let working = machine_working(
            &mut pool,
            root,
            &[],
            &References::new(),
            Domain::F64,
            &[&backend],
            Preference::Automatic,
        )
        .expect("working");

        let kinds: Vec<StepKind> = working.steps.iter().map(|step| step.kind).collect();
        assert_eq!(kinds, [StepKind::Machine(Domain::F64)]);
    }

    #[test]
    fn a_machine_step_holds_the_value_the_backend_computed() {
        let mut pool = ExprPool::new();
        let root = parsed(&mut pool, "0.1 + 0.2");
        let backend = CpuBackend::new();

        let working = machine_working(
            &mut pool,
            root,
            &[],
            &References::new(),
            Domain::F64,
            &[&backend],
            Preference::Automatic,
        )
        .expect("working");

        assert_eq!(
            working.steps[0].result,
            StepValue::Number(Number::F64(0.1_f64 + 0.2))
        );
    }

    #[test]
    fn an_approximate_operation_has_no_machine_steps() {
        let mut pool = ExprPool::new();
        let root = parsed(&mut pool, "sin(1) + 1");
        let backend = CpuBackend::new();

        let working = machine_working(
            &mut pool,
            root,
            &[],
            &References::new(),
            Domain::F64,
            &[&backend],
            Preference::Automatic,
        );

        assert_eq!(working, Err(WorkingError::ApproximateOperations));
    }

    #[test]
    fn an_exact_step_whose_value_is_symbolic_keeps_its_expression() {
        let mut pool = ExprPool::new();

        let working = exact(&mut pool, "pi * 2");

        assert!(matches!(
            working.steps[0].result,
            StepValue::Expression(_) | StepValue::Number(_)
        ));
    }
}
