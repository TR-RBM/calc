use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;

use calc_numbers::Number;
use calc_units::{UnitId, UnitTable};

use crate::ids::{ExprId, NumberId, SymbolId, next_index, position};
use crate::node::{BinderKind, Head, Node, NodeView};
use crate::symbol::{SymbolError, SymbolKind, SymbolTable};

const LAMBDA_ARGUMENTS: &[usize] = &[0];
const RANGE_ARGUMENTS: &[usize] = &[2];
const INTEGRAL_ARGUMENTS: &[usize] = &[0, 2];
const POINT_ARGUMENTS: &[usize] = &[1];
const SORT_ARGUMENTS: &[usize] = &[1, 2, 3];

const ANONYMOUS_MEASUREMENT: &str = "#m";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolTable {
    Nodes,
    Numbers,
    Symbols,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildError {
    UnknownExprId(ExprId),
    UnknownSymbolId(SymbolId),
    UnknownUnitId(UnitId),
    ArityMismatch { expected: usize, found: usize },
    NotAFunction(SymbolId),
    BinderArgumentCount { binder: BinderKind, found: usize },
    ShapeMismatch { expected: usize, found: usize },
    ShapeTooLarge,
    TableFull(PoolTable),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessError {
    UnknownExprId(ExprId),
    UnknownNumberId(NumberId),
    NotABinder(ExprId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactError {
    UnknownExprId(ExprId),
    UnknownNumberId(NumberId),
    TableFull(PoolTable),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompactionMap {
    expressions: HashMap<ExprId, ExprId>,
}

impl CompactionMap {
    pub fn get(&self, old: ExprId) -> Option<ExprId> {
        self.expressions.get(&old).copied()
    }
}

pub struct ExprPool {
    nodes: Vec<Node>,
    node_buckets: HashMap<u64, Vec<ExprId>>,
    hasher: RandomState,
    numbers: Vec<Number>,
    number_ids: HashMap<Number, NumberId>,
    symbols: SymbolTable,
    units: UnitTable,
    bound_names: HashMap<ExprId, String>,
    entry_limit: u32,
}

impl Default for ExprPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprPool {
    pub fn new() -> Self {
        Self::with_entry_limit(u32::MAX)
    }

    fn with_entry_limit(entry_limit: u32) -> Self {
        Self::with_parts(SymbolTable::new(entry_limit), UnitTable::new(), entry_limit)
    }

    fn with_parts(symbols: SymbolTable, units: UnitTable, entry_limit: u32) -> Self {
        Self {
            nodes: Vec::new(),
            node_buckets: HashMap::new(),
            hasher: RandomState::new(),
            numbers: Vec::new(),
            number_ids: HashMap::new(),
            symbols,
            units,
            bound_names: HashMap::new(),
            entry_limit,
        }
    }

    pub fn expression_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn units(&self) -> &UnitTable {
        &self.units
    }

    pub fn units_mut(&mut self) -> &mut UnitTable {
        &mut self.units
    }

    pub fn intern_symbol(&mut self, name: &str, kind: SymbolKind) -> Result<SymbolId, SymbolError> {
        self.symbols.intern(name, kind)
    }

    pub fn fresh_measurement(&mut self) -> Result<SymbolId, SymbolError> {
        self.symbols.fresh_measurement()
    }

    pub fn anonymous_measurement(&mut self) -> Result<ExprId, BuildError> {
        let symbol = self
            .intern_symbol(ANONYMOUS_MEASUREMENT, SymbolKind::Measurement)
            .map_err(|_| BuildError::TableFull(PoolTable::Symbols))?;
        self.symbol(symbol)
    }

    pub fn measurement(&mut self) -> Result<ExprId, BuildError> {
        let symbol = self
            .fresh_measurement()
            .map_err(|_| BuildError::TableFull(PoolTable::Symbols))?;
        self.symbol(symbol)
    }

    pub fn lookup_symbol(&self, name: &str) -> Option<SymbolId> {
        self.symbols.lookup(name)
    }

    pub fn symbol_name(&self, symbol: SymbolId) -> Result<&str, SymbolError> {
        self.symbols.name(symbol)
    }

    pub fn symbol_kind(&self, symbol: SymbolId) -> Result<SymbolKind, SymbolError> {
        self.symbols.kind(symbol)
    }

    pub fn node(&self, expression: ExprId) -> Result<NodeView<'_>, AccessError> {
        self.nodes
            .get(position(expression.0))
            .map(NodeView::from)
            .ok_or(AccessError::UnknownExprId(expression))
    }

    pub fn number_value(&self, number: NumberId) -> Result<&Number, AccessError> {
        self.numbers
            .get(position(number.0))
            .ok_or(AccessError::UnknownNumberId(number))
    }

    pub fn number(&mut self, value: Number) -> Result<ExprId, BuildError> {
        let number = self.intern_number(value)?;
        self.insert(Node::Number(number))
    }

    pub fn symbol(&mut self, symbol: SymbolId) -> Result<ExprId, BuildError> {
        self.symbols
            .kind(symbol)
            .map_err(|_| BuildError::UnknownSymbolId(symbol))?;
        self.insert(Node::Symbol(symbol))
    }

    pub fn bound(&mut self, index: u32) -> Result<ExprId, BuildError> {
        self.insert(Node::Bound(index))
    }

    pub fn apply(&mut self, head: Head, arguments: &[ExprId]) -> Result<ExprId, BuildError> {
        self.check_expressions(arguments)?;
        let expected = match head {
            Head::Operator(operator) => operator.arity(),
            Head::Function(symbol) => match self.symbols.kind(symbol) {
                Ok(SymbolKind::Function { arity }) => arity,
                Ok(_) => return Err(BuildError::NotAFunction(symbol)),
                Err(_) => return Err(BuildError::UnknownSymbolId(symbol)),
            },
        };
        if expected != arguments.len() {
            return Err(BuildError::ArityMismatch {
                expected,
                found: arguments.len(),
            });
        }
        self.insert(Node::Apply {
            head,
            arguments: arguments.into(),
        })
    }

    pub fn bind(
        &mut self,
        binder: BinderKind,
        arguments: &[ExprId],
        body: ExprId,
    ) -> Result<ExprId, BuildError> {
        self.check_expressions(arguments)?;
        self.check_expressions(&[body])?;
        if !allowed_binder_argument_counts(binder).contains(&arguments.len()) {
            return Err(BuildError::BinderArgumentCount {
                binder,
                found: arguments.len(),
            });
        }
        self.insert(Node::Bind {
            binder,
            arguments: arguments.into(),
            body,
        })
    }

    pub fn quantity(&mut self, value: ExprId, unit: UnitId) -> Result<ExprId, BuildError> {
        self.check_expressions(&[value])?;
        self.units
            .dimension(unit)
            .map_err(|_| BuildError::UnknownUnitId(unit))?;
        self.insert(Node::Quantity { value, unit })
    }

    pub fn array(&mut self, shape: &[u32], elements: &[ExprId]) -> Result<ExprId, BuildError> {
        self.check_expressions(elements)?;
        let expected = shape
            .iter()
            .try_fold(1_usize, |product, length| {
                usize::try_from(*length)
                    .ok()
                    .and_then(|length| product.checked_mul(length))
            })
            .ok_or(BuildError::ShapeTooLarge)?;
        if expected != elements.len() {
            return Err(BuildError::ShapeMismatch {
                expected,
                found: elements.len(),
            });
        }
        self.insert(Node::Array {
            shape: shape.into(),
            elements: elements.into(),
        })
    }

    pub fn record_bound_name(&mut self, binder: ExprId, name: &str) -> Result<(), AccessError> {
        match self.nodes.get(position(binder.0)) {
            Some(Node::Bind { .. }) => {
                self.bound_names
                    .entry(binder)
                    .or_insert_with(|| name.to_string());
                Ok(())
            }
            Some(_) => Err(AccessError::NotABinder(binder)),
            None => Err(AccessError::UnknownExprId(binder)),
        }
    }

    pub fn bound_name(&self, binder: ExprId) -> Option<&str> {
        self.bound_names.get(&binder).map(String::as_str)
    }

    pub fn compact(self, roots: &[ExprId]) -> Result<(ExprPool, CompactionMap), CompactError> {
        let reachable = self.reachable_from(roots)?;
        let mut compacted = ExprPool::with_parts(self.symbols, self.units, self.entry_limit);
        let mut map = CompactionMap::default();
        for (old, node) in self.nodes.iter().enumerate() {
            if !reachable.get(old).copied().unwrap_or(false) {
                continue;
            }
            let old_id = ExprId(u32::try_from(old).unwrap_or(u32::MAX));
            let new_node = remap_node(node, &map, &self.numbers, &mut compacted)?;
            let new_id = compacted
                .insert(new_node)
                .map_err(|_| CompactError::TableFull(PoolTable::Nodes))?;
            map.expressions.insert(old_id, new_id);
            if let Some(name) = self.bound_names.get(&old_id) {
                compacted.bound_names.insert(new_id, name.clone());
            }
        }
        Ok((compacted, map))
    }

    fn reachable_from(&self, roots: &[ExprId]) -> Result<Vec<bool>, CompactError> {
        let mut reachable = vec![false; self.nodes.len()];
        let mut pending = roots.to_vec();
        while let Some(expression) = pending.pop() {
            let index = position(expression.0);
            let node = self
                .nodes
                .get(index)
                .ok_or(CompactError::UnknownExprId(expression))?;
            if let Some(mark) = reachable.get_mut(index)
                && !*mark
            {
                *mark = true;
                pending.extend(node.children());
            }
        }
        Ok(reachable)
    }

    fn check_expressions(&self, expressions: &[ExprId]) -> Result<(), BuildError> {
        match expressions
            .iter()
            .find(|expression| position(expression.0) >= self.nodes.len())
        {
            Some(unknown) => Err(BuildError::UnknownExprId(*unknown)),
            None => Ok(()),
        }
    }

    fn intern_number(&mut self, value: Number) -> Result<NumberId, BuildError> {
        if let Some(existing) = self.number_ids.get(&value) {
            return Ok(*existing);
        }
        let index = next_index(self.numbers.len(), self.entry_limit)
            .ok_or(BuildError::TableFull(PoolTable::Numbers))?;
        let number = NumberId(index);
        self.numbers.push(value.clone());
        self.number_ids.insert(value, number);
        Ok(number)
    }

    fn insert(&mut self, node: Node) -> Result<ExprId, BuildError> {
        let hash = self.hasher.hash_one(&node);
        if let Some(existing) = self.node_buckets.get(&hash).and_then(|bucket| {
            bucket
                .iter()
                .copied()
                .find(|candidate| self.nodes.get(position(candidate.0)) == Some(&node))
        }) {
            return Ok(existing);
        }
        let index = next_index(self.nodes.len(), self.entry_limit)
            .ok_or(BuildError::TableFull(PoolTable::Nodes))?;
        let expression = ExprId(index);
        self.nodes.push(node);
        self.node_buckets.entry(hash).or_default().push(expression);
        Ok(expression)
    }
}

fn allowed_binder_argument_counts(binder: BinderKind) -> &'static [usize] {
    match binder {
        BinderKind::Lambda => LAMBDA_ARGUMENTS,
        BinderKind::Sum(_) | BinderKind::Product(_) | BinderKind::Taylor => RANGE_ARGUMENTS,
        BinderKind::Integral => INTEGRAL_ARGUMENTS,
        BinderKind::Limit(_) | BinderKind::Derivative | BinderKind::Root => POINT_ARGUMENTS,
        BinderKind::Sort(_) => SORT_ARGUMENTS,
    }
}

fn remap_node(
    node: &Node,
    map: &CompactionMap,
    old_numbers: &[Number],
    compacted: &mut ExprPool,
) -> Result<Node, CompactError> {
    let remap = |expression: &ExprId| {
        map.get(*expression)
            .ok_or(CompactError::UnknownExprId(*expression))
    };
    Ok(match node {
        Node::Number(number) => {
            let value = old_numbers
                .get(position(number.0))
                .ok_or(CompactError::UnknownNumberId(*number))?;
            let new_number = compacted
                .intern_number(value.clone())
                .map_err(|_| CompactError::TableFull(PoolTable::Numbers))?;
            Node::Number(new_number)
        }
        Node::Symbol(symbol) => Node::Symbol(*symbol),
        Node::Bound(index) => Node::Bound(*index),
        Node::Apply { head, arguments } => Node::Apply {
            head: *head,
            arguments: arguments.iter().map(remap).collect::<Result<_, _>>()?,
        },
        Node::Bind {
            binder,
            arguments,
            body,
        } => Node::Bind {
            binder: *binder,
            arguments: arguments.iter().map(remap).collect::<Result<_, _>>()?,
            body: remap(body)?,
        },
        Node::Quantity { value, unit } => Node::Quantity {
            value: remap(value)?,
            unit: *unit,
        },
        Node::Array { shape, elements } => Node::Array {
            shape: shape.clone(),
            elements: elements.iter().map(remap).collect::<Result<_, _>>()?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{LimitSide, ReductionShape};
    use crate::operator::Operator;
    use crate::symbol::BuiltinConstant;
    use calc_numbers::Integer;

    const SMALL_LIMIT: u32 = 2;

    fn integer(pool: &mut ExprPool, value: i64) -> ExprId {
        pool.number(Number::from(value)).unwrap()
    }

    fn variable(pool: &mut ExprPool, name: &str) -> ExprId {
        let symbol = pool.intern_symbol(name, SymbolKind::Variable).unwrap();
        pool.symbol(symbol).unwrap()
    }

    fn add(pool: &mut ExprPool, left: ExprId, right: ExprId) -> ExprId {
        pool.apply(Head::Operator(Operator::Add), &[left, right])
            .unwrap()
    }

    #[test]
    fn structurally_equal_nodes_share_one_id() {
        let mut pool = ExprPool::new();
        let first_x = variable(&mut pool, "x");
        let one = integer(&mut pool, 1);
        let first = add(&mut pool, first_x, one);
        let second_x = variable(&mut pool, "x");
        let second = add(&mut pool, second_x, one);
        assert_eq!(first, second);
    }

    #[test]
    fn adding_an_equal_node_does_not_grow_the_pool() {
        let mut pool = ExprPool::new();
        integer(&mut pool, 7);
        let count = pool.expression_count();
        integer(&mut pool, 7);
        assert_eq!(pool.expression_count(), count);
    }

    #[test]
    fn swapped_arguments_give_a_different_node() {
        let mut pool = ExprPool::new();
        let a = variable(&mut pool, "a");
        let b = variable(&mut pool, "b");
        let forward = add(&mut pool, a, b);
        let backward = add(&mut pool, b, a);
        assert_ne!(forward, backward);
    }

    #[test]
    fn written_association_of_a_sum_is_kept() {
        let mut pool = ExprPool::new();
        let a = variable(&mut pool, "a");
        let b = variable(&mut pool, "b");
        let c = variable(&mut pool, "c");
        let a_plus_b = add(&mut pool, a, b);
        let left = add(&mut pool, a_plus_b, c);
        let b_plus_c = add(&mut pool, b, c);
        let right = add(&mut pool, a, b_plus_c);
        assert_ne!(left, right);
    }

    #[test]
    fn positive_and_negative_zero_are_different_numbers() {
        let mut pool = ExprPool::new();
        let positive = pool.number(Number::F64(0.0)).unwrap();
        let negative = pool.number(Number::F64(-0.0)).unwrap();
        assert_ne!(positive, negative);
    }

    #[test]
    fn nan_with_equal_bits_is_one_number() {
        let mut pool = ExprPool::new();
        let first = pool.number(Number::F64(f64::NAN)).unwrap();
        let second = pool.number(Number::F64(f64::NAN)).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn f32_and_f64_with_equal_value_are_different_numbers() {
        let mut pool = ExprPool::new();
        let single = pool.number(Number::F32(1.5)).unwrap();
        let double = pool.number(Number::F64(1.5)).unwrap();
        assert_ne!(single, double);
    }

    #[test]
    fn number_node_reads_back_its_value() {
        let mut pool = ExprPool::new();
        let expression = integer(&mut pool, 42);
        let NodeView::Number(number) = pool.node(expression).unwrap() else {
            panic!("expected a number node");
        };
        assert_eq!(
            pool.number_value(number).unwrap(),
            &Number::Integer(Integer::from(42_i64))
        );
    }

    #[test]
    fn builtin_constants_are_registered_under_their_names() {
        let pool = ExprPool::new();
        for constant in BuiltinConstant::ALL {
            assert_eq!(pool.lookup_symbol(constant.name()), Some(constant.symbol()));
        }
    }

    #[test]
    fn builtin_constants_have_constant_kind() {
        let pool = ExprPool::new();
        assert_eq!(
            pool.symbol_kind(BuiltinConstant::Pi.symbol()).unwrap(),
            SymbolKind::Constant
        );
    }

    #[test]
    fn interning_a_name_twice_with_the_same_kind_gives_one_symbol() {
        let mut pool = ExprPool::new();
        let first = pool.intern_symbol("mass", SymbolKind::Variable).unwrap();
        let second = pool.intern_symbol("mass", SymbolKind::Variable).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn interning_a_name_with_another_kind_is_a_conflict() {
        let mut pool = ExprPool::new();
        let symbol = pool.intern_symbol("f", SymbolKind::Variable).unwrap();
        assert_eq!(
            pool.intern_symbol("f", SymbolKind::Function { arity: 1 }),
            Err(SymbolError::KindConflict {
                symbol,
                existing: SymbolKind::Variable
            })
        );
    }

    #[test]
    fn a_builtin_constant_name_cannot_become_a_variable() {
        let mut pool = ExprPool::new();
        assert!(matches!(
            pool.intern_symbol("pi", SymbolKind::Variable),
            Err(SymbolError::KindConflict { .. })
        ));
    }

    #[test]
    fn symbol_name_reads_back_the_interned_name() {
        let mut pool = ExprPool::new();
        let symbol = pool
            .intern_symbol("velocity", SymbolKind::Variable)
            .unwrap();
        assert_eq!(pool.symbol_name(symbol).unwrap(), "velocity");
    }

    #[test]
    fn symbol_node_for_an_unknown_symbol_is_rejected() {
        let mut pool = ExprPool::new();
        let foreign = SymbolId(1000);
        assert_eq!(
            pool.symbol(foreign),
            Err(BuildError::UnknownSymbolId(foreign))
        );
    }

    #[test]
    fn operator_with_wrong_argument_count_is_rejected() {
        let mut pool = ExprPool::new();
        let x = variable(&mut pool, "x");
        assert_eq!(
            pool.apply(Head::Operator(Operator::Add), &[x]),
            Err(BuildError::ArityMismatch {
                expected: 2,
                found: 1
            })
        );
    }

    #[test]
    fn user_function_is_applied_with_its_arity() {
        let mut pool = ExprPool::new();
        let f = pool
            .intern_symbol("f", SymbolKind::Function { arity: 2 })
            .unwrap();
        let x = variable(&mut pool, "x");
        let y = variable(&mut pool, "y");
        let applied = pool.apply(Head::Function(f), &[x, y]).unwrap();
        assert_eq!(
            pool.node(applied).unwrap(),
            NodeView::Apply {
                head: Head::Function(f),
                arguments: &[x, y]
            }
        );
    }

    #[test]
    fn a_variable_cannot_be_the_head_of_an_application() {
        let mut pool = ExprPool::new();
        let g = pool.intern_symbol("g", SymbolKind::Variable).unwrap();
        let x = variable(&mut pool, "x");
        assert_eq!(
            pool.apply(Head::Function(g), &[x]),
            Err(BuildError::NotAFunction(g))
        );
    }

    #[test]
    fn argument_from_outside_the_pool_is_rejected() {
        let mut pool = ExprPool::new();
        let foreign = ExprId(99);
        assert_eq!(
            pool.apply(Head::Operator(Operator::Neg), &[foreign]),
            Err(BuildError::UnknownExprId(foreign))
        );
    }

    #[test]
    fn alpha_equivalent_lambdas_share_one_id() {
        let mut pool = ExprPool::new();
        let bound = pool.bound(0).unwrap();
        let first = pool.bind(BinderKind::Lambda, &[], bound).unwrap();
        pool.record_bound_name(first, "x").unwrap();
        let second = pool.bind(BinderKind::Lambda, &[], bound).unwrap();
        pool.record_bound_name(second, "t").unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn first_recorded_bound_name_is_kept() {
        let mut pool = ExprPool::new();
        let bound = pool.bound(0).unwrap();
        let lambda = pool.bind(BinderKind::Lambda, &[], bound).unwrap();
        pool.record_bound_name(lambda, "x").unwrap();
        pool.record_bound_name(lambda, "t").unwrap();
        assert_eq!(pool.bound_name(lambda), Some("x"));
    }

    #[test]
    fn bound_name_on_a_node_that_is_not_a_binder_is_rejected() {
        let mut pool = ExprPool::new();
        let x = variable(&mut pool, "x");
        assert_eq!(
            pool.record_bound_name(x, "x"),
            Err(AccessError::NotABinder(x))
        );
    }

    #[test]
    fn sums_with_different_reduction_shapes_are_different_nodes() {
        let mut pool = ExprPool::new();
        let one = integer(&mut pool, 1);
        let ten = integer(&mut pool, 10);
        let body = pool.bound(0).unwrap();
        let left_fold = pool
            .bind(BinderKind::Sum(ReductionShape::LeftFold), &[one, ten], body)
            .unwrap();
        let halving = pool
            .bind(BinderKind::Sum(ReductionShape::Halving), &[one, ten], body)
            .unwrap();
        assert_ne!(left_fold, halving);
    }

    #[test]
    fn binder_with_wrong_argument_count_is_rejected() {
        let mut pool = ExprPool::new();
        let point = integer(&mut pool, 0);
        let body = pool.bound(0).unwrap();
        let binder = BinderKind::Sum(ReductionShape::LeftFold);
        assert_eq!(
            pool.bind(binder, &[point], body),
            Err(BuildError::BinderArgumentCount { binder, found: 1 })
        );
    }

    #[test]
    fn integral_accepts_no_bounds_or_two_bounds() {
        let mut pool = ExprPool::new();
        let lower = integer(&mut pool, 0);
        let upper = integer(&mut pool, 1);
        let body = pool.bound(0).unwrap();
        assert!(pool.bind(BinderKind::Integral, &[], body).is_ok());
        assert!(
            pool.bind(BinderKind::Integral, &[lower, upper], body)
                .is_ok()
        );
    }

    #[test]
    fn limit_takes_one_point() {
        let mut pool = ExprPool::new();
        let point = integer(&mut pool, 0);
        let body = pool.bound(0).unwrap();
        assert!(
            pool.bind(BinderKind::Limit(LimitSide::Left), &[point], body)
                .is_ok()
        );
    }

    #[test]
    fn quantity_reads_back_value_and_unit() {
        let mut pool = ExprPool::new();
        let value = integer(&mut pool, 3);
        let metre = pool.units_mut().lookup("m").unwrap();
        let quantity = pool.quantity(value, metre).unwrap();
        assert_eq!(
            pool.node(quantity).unwrap(),
            NodeView::Quantity { value, unit: metre }
        );
    }

    #[test]
    fn quantity_with_a_unit_from_another_table_is_rejected() {
        let mut pool = ExprPool::new();
        let mut other_table = UnitTable::new();
        for name in ["km", "mm", "cm", "dm", "nm", "pm"] {
            other_table.lookup(name).unwrap();
        }
        let foreign = other_table.lookup("fm").unwrap();
        let value = integer(&mut pool, 3);
        assert_eq!(
            pool.quantity(value, foreign),
            Err(BuildError::UnknownUnitId(foreign))
        );
    }

    #[test]
    fn array_keeps_shape_and_row_major_elements() {
        let mut pool = ExprPool::new();
        let elements: Vec<ExprId> = (1..=4).map(|value| integer(&mut pool, value)).collect();
        let matrix = pool.array(&[2, 2], &elements).unwrap();
        assert_eq!(
            pool.node(matrix).unwrap(),
            NodeView::Array {
                shape: &[2, 2],
                elements: &elements
            }
        );
    }

    #[test]
    fn array_with_element_count_not_matching_shape_is_rejected() {
        let mut pool = ExprPool::new();
        let elements: Vec<ExprId> = (1..=3).map(|value| integer(&mut pool, value)).collect();
        assert_eq!(
            pool.array(&[2, 2], &elements),
            Err(BuildError::ShapeMismatch {
                expected: 4,
                found: 3
            })
        );
    }

    #[test]
    fn array_shape_whose_size_overflows_is_rejected() {
        let mut pool = ExprPool::new();
        assert_eq!(
            pool.array(&[u32::MAX, u32::MAX, u32::MAX], &[]),
            Err(BuildError::ShapeTooLarge)
        );
    }

    #[test]
    fn reading_an_unknown_expression_is_an_error() {
        let pool = ExprPool::new();
        let foreign = ExprId(5);
        assert_eq!(pool.node(foreign), Err(AccessError::UnknownExprId(foreign)));
    }

    #[test]
    fn reading_an_unknown_number_is_an_error() {
        let pool = ExprPool::new();
        let foreign = NumberId(5);
        assert_eq!(
            pool.number_value(foreign),
            Err(AccessError::UnknownNumberId(foreign))
        );
    }

    #[test]
    fn full_node_table_rejects_a_new_node() {
        let mut pool = ExprPool::with_entry_limit(SMALL_LIMIT);
        pool.bound(0).unwrap();
        pool.bound(1).unwrap();
        assert_eq!(pool.bound(2), Err(BuildError::TableFull(PoolTable::Nodes)));
    }

    #[test]
    fn full_number_table_rejects_a_new_number() {
        let mut pool = ExprPool::with_entry_limit(SMALL_LIMIT);
        pool.number(Number::from(1_i64)).unwrap();
        pool.number(Number::from(2_i64)).unwrap();
        assert_eq!(
            pool.number(Number::from(3_i64)),
            Err(BuildError::TableFull(PoolTable::Numbers))
        );
    }

    #[test]
    fn full_symbol_table_rejects_a_new_symbol() {
        let mut pool =
            ExprPool::with_entry_limit(u32::try_from(BuiltinConstant::ALL.len()).unwrap());
        assert_eq!(
            pool.intern_symbol("x", SymbolKind::Variable),
            Err(SymbolError::TableFull)
        );
    }

    #[test]
    fn compaction_keeps_the_structure_of_a_root() {
        let mut pool = ExprPool::new();
        let x = variable(&mut pool, "x");
        let two = integer(&mut pool, 2);
        let root = pool
            .apply(Head::Operator(Operator::Pow), &[x, two])
            .unwrap();
        let (compacted, map) = pool.compact(&[root]).unwrap();
        let new_root = map.get(root).unwrap();
        assert_eq!(
            compacted.node(new_root).unwrap(),
            NodeView::Apply {
                head: Head::Operator(Operator::Pow),
                arguments: &[map.get(x).unwrap(), map.get(two).unwrap()]
            }
        );
    }

    #[test]
    fn compaction_drops_unreachable_nodes() {
        let mut pool = ExprPool::new();
        let kept = integer(&mut pool, 1);
        let dropped = integer(&mut pool, 2);
        let (compacted, map) = pool.compact(&[kept]).unwrap();
        assert_eq!(map.get(dropped), None);
        assert_eq!(compacted.expression_count(), 1);
    }

    #[test]
    fn compaction_keeps_number_values() {
        let mut pool = ExprPool::new();
        integer(&mut pool, 5);
        let kept = pool.number(Number::F64(-0.0)).unwrap();
        let (compacted, map) = pool.compact(&[kept]).unwrap();
        let NodeView::Number(number) = compacted.node(map.get(kept).unwrap()).unwrap() else {
            panic!("expected a number node");
        };
        assert_eq!(compacted.number_value(number).unwrap(), &Number::F64(-0.0));
    }

    #[test]
    fn compaction_keeps_bound_names_of_reachable_binders() {
        let mut pool = ExprPool::new();
        let body = pool.bound(0).unwrap();
        let lambda = pool.bind(BinderKind::Lambda, &[], body).unwrap();
        pool.record_bound_name(lambda, "t").unwrap();
        let (compacted, map) = pool.compact(&[lambda]).unwrap();
        assert_eq!(compacted.bound_name(map.get(lambda).unwrap()), Some("t"));
    }

    #[test]
    fn two_marks_stay_two_after_compaction() {
        let mut pool = ExprPool::new();
        let value = pool.number(Number::from(10_i64)).unwrap();
        let spread = pool.number(Number::from(1_i64)).unwrap();
        let first_mark = pool.measurement().unwrap();
        let second_mark = pool.measurement().unwrap();
        let first = pool
            .apply(
                Head::Operator(Operator::Uncertain),
                &[value, spread, first_mark],
            )
            .unwrap();
        let second = pool
            .apply(
                Head::Operator(Operator::Uncertain),
                &[value, spread, second_mark],
            )
            .unwrap();

        let (_, map) = pool.compact(&[first, second]).unwrap();

        assert_ne!(map.get(first), map.get(second));
    }

    #[test]
    fn compaction_keeps_symbol_ids() {
        let mut pool = ExprPool::new();
        let symbol = pool.intern_symbol("y", SymbolKind::Variable).unwrap();
        let root = pool.symbol(symbol).unwrap();
        let (compacted, map) = pool.compact(&[root]).unwrap();
        assert_eq!(
            compacted.node(map.get(root).unwrap()).unwrap(),
            NodeView::Symbol(symbol)
        );
    }

    #[test]
    fn compaction_with_an_unknown_root_is_an_error() {
        let pool = ExprPool::new();
        let foreign = ExprId(3);
        assert!(matches!(
            pool.compact(&[foreign]),
            Err(CompactError::UnknownExprId(id)) if id == foreign
        ));
    }

    #[test]
    fn compacted_pool_still_hash_conses() {
        let mut pool = ExprPool::new();
        let root = integer(&mut pool, 8);
        let (mut compacted, map) = pool.compact(&[root]).unwrap();
        let again = compacted.number(Number::from(8_i64)).unwrap();
        assert_eq!(map.get(root), Some(again));
    }

    #[test]
    fn pool_can_be_shared_across_threads_for_reading() {
        fn assert_send_and_sync<T: Send + Sync>() {}
        assert_send_and_sync::<ExprPool>();
    }
}
