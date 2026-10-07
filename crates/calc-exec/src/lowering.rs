use std::collections::HashMap;

use calc_expr::{
    AccessError, BinderKind, BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator,
    ReductionShape, SymbolId,
};
use calc_numbers::Number;

use crate::domain::{Constant, Domain};
use crate::operation::{PlanOp, ValueKind};
use crate::plan::{ChannelId, Gate, GateId, Plan, PlanError, Reduce, ReduceOperation, ReduceShape};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueForm {
    Real,
    Complex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LowerInput {
    pub symbol: SymbolId,
    pub form: ValueForm,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LowerSpec {
    pub domain: Domain,
    pub inputs: Vec<LowerInput>,
    pub output: ValueForm,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LowerError {
    Access(AccessError),
    DuplicateInput(SymbolId),
    UnboundSymbol(SymbolId),
    UnexpectedBound(u32),
    UnsupportedOperator(Operator),
    UnsupportedFunction(SymbolId),
    UnsupportedBinder(BinderKind),
    NonConstantBounds(ExprId),
    EmptyRange(ExprId),
    QuantityNotConverted(ExprId),
    ArrayNotSupported(ExprId),
    MixedMachineDomain {
        expression: ExprId,
        found: Domain,
    },
    ScalarExpected(ExprId),
    BooleanExpected(ExprId),
    ComplexNotSupported(Operator),
    ComplexReduction(ExprId),
    OutputFormMismatch {
        requested: ValueForm,
        found: ValueForm,
    },
    Plan(PlanError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Scalar(usize),
    Boolean(usize),
    Complex { real: usize, imaginary: usize },
}

struct Lowering<'pool> {
    pool: &'pool ExprPool,
    domain: Domain,
    gates: Vec<Gate>,
    inputs: HashMap<SymbolId, Value>,
    bound: Option<usize>,
    memo: HashMap<ExprId, Value>,
}

fn gate_id(index: usize) -> GateId {
    GateId(u32::try_from(index).unwrap_or(u32::MAX))
}

fn channel_id(index: usize) -> ChannelId {
    ChannelId(u32::try_from(index).unwrap_or(u32::MAX))
}

fn plan_operation(operator: Operator) -> Option<PlanOp> {
    let operation = match operator {
        Operator::Add => PlanOp::Add,
        Operator::Sub => PlanOp::Sub,
        Operator::Mul => PlanOp::Mul,
        Operator::Div => PlanOp::Div,
        Operator::Neg => PlanOp::Neg,
        Operator::Pow => PlanOp::Pow,
        Operator::Sqrt => PlanOp::Sqrt,
        Operator::Abs => PlanOp::Abs,
        Operator::MulAdd => PlanOp::MulAdd,
        Operator::Floor => PlanOp::Floor,
        Operator::Ceil => PlanOp::Ceil,
        Operator::Trunc => PlanOp::Trunc,
        Operator::RoundTiesEven => PlanOp::RoundTiesEven,
        Operator::CopySign => PlanOp::CopySign,
        Operator::Min => PlanOp::Min,
        Operator::Max => PlanOp::Max,
        Operator::Exp => PlanOp::Exp,
        Operator::Ln => PlanOp::Ln,
        Operator::Sin => PlanOp::Sin,
        Operator::Cos => PlanOp::Cos,
        Operator::Tan => PlanOp::Tan,
        Operator::Asin => PlanOp::Asin,
        Operator::Acos => PlanOp::Acos,
        Operator::Atan => PlanOp::Atan,
        Operator::Atan2 => PlanOp::Atan2,
        Operator::Equal => PlanOp::Equal,
        Operator::NotEqual => PlanOp::NotEqual,
        Operator::Less => PlanOp::Less,
        Operator::LessOrEqual => PlanOp::LessOrEqual,
        Operator::Greater => PlanOp::Greater,
        Operator::GreaterOrEqual => PlanOp::GreaterOrEqual,
        Operator::And => PlanOp::And,
        Operator::Or => PlanOp::Or,
        Operator::Not => PlanOp::Not,
        Operator::Select => PlanOp::Select,
        Operator::EnclosureLower
        | Operator::EnclosureUpper
        | Operator::RationalPart
        | Operator::CoefficientOf
        | Operator::RealPart
        | Operator::ImaginaryPart
        | Operator::Conjugate
        | Operator::Factorial
        | Operator::Percent
        | Operator::Round
        | Operator::Binomial
        | Operator::Sinh
        | Operator::Cosh
        | Operator::Tanh
        | Operator::Log
        | Operator::Log2
        | Operator::Log10
        | Operator::Gcd
        | Operator::Count
        | Operator::Total
        | Operator::Mean
        | Operator::Median
        | Operator::Transpose
        | Operator::Determinant
        | Operator::Trace
        | Operator::Inverse
        | Operator::Rank
        | Operator::Eigenvalues
        | Operator::Kernel
        | Operator::Eigenvectors
        | Operator::At
        | Operator::Smallest
        | Operator::Largest
        | Operator::Sorted
        | Operator::Lcm
        | Operator::Mod
        | Operator::Complex
        | Operator::ToF32
        | Operator::ToF64
        | Operator::ToExact
        | Operator::Uncertain
        | Operator::UncertainExpanded
        | Operator::ConvertUnit
        | Operator::FromCelsius
        | Operator::FromFahrenheit
        | Operator::ToCelsius
        | Operator::ToFahrenheit
        | Operator::BitAnd
        | Operator::BitOr
        | Operator::BitXor
        | Operator::ShiftLeft
        | Operator::ShiftRight
        | Operator::BitNot
        | Operator::Wrap
        | Operator::InType
        | Operator::Radix
        | Operator::Bytes
        | Operator::Between
        | Operator::Tolerance
        | Operator::Substance
        | Operator::Reaction
        | Operator::MolarMass
        | Operator::Nuclide
        | Operator::NuclearReaction
        | Operator::QValue
        | Operator::Philox4x32_10 => return None,
    };
    Some(operation)
}

fn reduce_shape(shape: ReductionShape) -> ReduceShape {
    match shape {
        ReductionShape::LeftFold => ReduceShape::LeftFold,
        ReductionShape::Halving => ReduceShape::Halving,
    }
}

impl<'pool> Lowering<'pool> {
    fn new(pool: &'pool ExprPool, spec: &LowerSpec) -> Result<Lowering<'pool>, LowerError> {
        let mut lowering = Lowering {
            pool,
            domain: spec.domain,
            gates: Vec::new(),
            inputs: HashMap::new(),
            bound: None,
            memo: HashMap::new(),
        };
        let mut channel = 0;
        for input in &spec.inputs {
            let value = match input.form {
                ValueForm::Real => {
                    channel += 1;
                    Value::Scalar(lowering.push(Gate::Input(channel_id(channel - 1))))
                }
                ValueForm::Complex => {
                    channel += 2;
                    Value::Complex {
                        real: lowering.push(Gate::Input(channel_id(channel - 2))),
                        imaginary: lowering.push(Gate::Input(channel_id(channel - 1))),
                    }
                }
            };
            if lowering.inputs.insert(input.symbol, value).is_some() {
                return Err(LowerError::DuplicateInput(input.symbol));
            }
        }
        Ok(lowering)
    }

    fn channel_count(&self) -> usize {
        self.gates
            .iter()
            .filter(|gate| matches!(gate, Gate::Input(_)))
            .count()
    }

    fn push(&mut self, gate: Gate) -> usize {
        self.gates.push(gate);
        self.gates.len() - 1
    }

    fn constant_f32_or_f64(&mut self, single: f32, double: f64) -> usize {
        let constant = match self.domain {
            Domain::F32 => Constant::F32(single),
            Domain::F64 => Constant::F64(double),
        };
        self.push(Gate::Constant(constant))
    }

    fn per_hundred(&mut self, values: &[Value]) -> Result<Value, LowerError> {
        let [Value::Scalar(argument)] = values else {
            return Err(LowerError::UnsupportedOperator(Operator::Percent));
        };
        let argument = *argument;
        let hundred = self.constant_f32_or_f64(100.0, 100.0);
        Ok(Value::Scalar(
            self.operation(PlanOp::Div, &[argument, hundred]),
        ))
    }

    fn hyperbolic(&mut self, operator: Operator, values: &[Value]) -> Result<Value, LowerError> {
        let [Value::Scalar(argument)] = values else {
            return Err(LowerError::UnsupportedOperator(operator));
        };
        let argument = *argument;
        let raised = self.operation(PlanOp::Exp, &[argument]);
        let opposite = self.operation(PlanOp::Neg, &[argument]);
        let lowered = self.operation(PlanOp::Exp, &[opposite]);
        let difference = self.operation(PlanOp::Sub, &[raised, lowered]);
        let total = self.operation(PlanOp::Add, &[raised, lowered]);
        let two = self.constant_f32_or_f64(2.0, 2.0);
        let gate = match operator {
            Operator::Sinh => self.operation(PlanOp::Div, &[difference, two]),
            Operator::Cosh => self.operation(PlanOp::Div, &[total, two]),
            _ => self.operation(PlanOp::Div, &[difference, total]),
        };
        Ok(Value::Scalar(gate))
    }

    fn natural_logarithm(&mut self, value: &Value) -> Result<usize, LowerError> {
        match value {
            Value::Scalar(gate) => Ok(self.operation(PlanOp::Ln, &[*gate])),
            Value::Boolean(_) | Value::Complex { .. } => {
                Err(LowerError::UnsupportedOperator(Operator::Log))
            }
        }
    }

    fn logarithm_to_base(&mut self, values: &[Value], base: f64) -> Result<Value, LowerError> {
        let [argument] = values else {
            return Err(LowerError::UnsupportedOperator(Operator::Log));
        };
        let numerator = self.natural_logarithm(argument)?;
        #[allow(clippy::cast_possible_truncation)]
        let base_gate = self.constant_f32_or_f64(base as f32, base);
        let denominator = self.operation(PlanOp::Ln, &[base_gate]);
        Ok(Value::Scalar(
            self.operation(PlanOp::Div, &[numerator, denominator]),
        ))
    }

    fn logarithm_of_values(&mut self, values: &[Value]) -> Result<Value, LowerError> {
        let [argument, base] = values else {
            return Err(LowerError::UnsupportedOperator(Operator::Log));
        };
        let numerator = self.natural_logarithm(argument)?;
        let denominator = self.natural_logarithm(base)?;
        Ok(Value::Scalar(
            self.operation(PlanOp::Div, &[numerator, denominator]),
        ))
    }

    fn operation(&mut self, operation: PlanOp, arguments: &[usize]) -> usize {
        self.push(Gate::Operation {
            operation,
            arguments: arguments.iter().copied().map(gate_id).collect(),
        })
    }

    fn value(&mut self, expression: ExprId) -> Result<Value, LowerError> {
        if let Some(value) = self.memo.get(&expression) {
            return Ok(*value);
        }
        let value = match self.pool.node(expression).map_err(LowerError::Access)? {
            NodeView::Number(number) => self.number(expression, number)?,
            NodeView::Symbol(symbol) => self.symbol(symbol)?,
            NodeView::Bound(index) => match (index, self.bound) {
                (0, Some(gate)) => Value::Scalar(gate),
                _ => return Err(LowerError::UnexpectedBound(index)),
            },
            NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            } => self.apply(expression, operator, arguments)?,
            NodeView::Apply {
                head: Head::Function(symbol),
                ..
            } => return Err(LowerError::UnsupportedFunction(symbol)),
            NodeView::Bind { binder, .. } => return Err(LowerError::UnsupportedBinder(binder)),
            NodeView::Quantity { .. } => return Err(LowerError::QuantityNotConverted(expression)),
            NodeView::Array { .. } => return Err(LowerError::ArrayNotSupported(expression)),
        };
        self.memo.insert(expression, value);
        Ok(value)
    }

    fn number(
        &mut self,
        expression: ExprId,
        number: calc_expr::NumberId,
    ) -> Result<Value, LowerError> {
        let value = self.pool.number_value(number).map_err(LowerError::Access)?;
        let constant = match (self.domain, value) {
            (Domain::F32, Number::F32(single)) => Constant::F32(*single),
            (Domain::F64, Number::F64(double)) => Constant::F64(*double),
            (Domain::F32, Number::F64(_)) => {
                return Err(LowerError::MixedMachineDomain {
                    expression,
                    found: Domain::F64,
                });
            }
            (Domain::F64, Number::F32(_)) => {
                return Err(LowerError::MixedMachineDomain {
                    expression,
                    found: Domain::F32,
                });
            }
            (Domain::F32, exact) => Constant::F32(exact.round_to_f32_ties_even()),
            (Domain::F64, exact) => Constant::F64(exact.round_to_f64_ties_even()),
        };
        Ok(Value::Scalar(self.push(Gate::Constant(constant))))
    }

    fn symbol(&mut self, symbol: SymbolId) -> Result<Value, LowerError> {
        if let Some(value) = self.inputs.get(&symbol) {
            return Ok(*value);
        }
        let value = if symbol == BuiltinConstant::Pi.symbol() {
            Value::Scalar(self.constant_f32_or_f64(std::f32::consts::PI, std::f64::consts::PI))
        } else if symbol == BuiltinConstant::E.symbol() {
            Value::Scalar(self.constant_f32_or_f64(std::f32::consts::E, std::f64::consts::E))
        } else if symbol == BuiltinConstant::Infinity.symbol() {
            Value::Scalar(self.constant_f32_or_f64(f32::INFINITY, f64::INFINITY))
        } else if symbol == BuiltinConstant::ImaginaryUnit.symbol() {
            Value::Complex {
                real: self.constant_f32_or_f64(0.0, 0.0),
                imaginary: self.constant_f32_or_f64(1.0, 1.0),
            }
        } else {
            return Err(LowerError::UnboundSymbol(symbol));
        };
        Ok(value)
    }

    fn apply(
        &mut self,
        expression: ExprId,
        operator: Operator,
        arguments: &[ExprId],
    ) -> Result<Value, LowerError> {
        if matches!(operator, Operator::Uncertain | Operator::UncertainExpanded) {
            return Err(LowerError::UnsupportedOperator(operator));
        }
        let values = arguments
            .iter()
            .map(|argument| self.value(*argument))
            .collect::<Result<Vec<Value>, LowerError>>()?;
        match operator {
            Operator::Percent => return self.per_hundred(&values),
            Operator::Sinh | Operator::Cosh | Operator::Tanh => {
                return self.hyperbolic(operator, &values);
            }
            Operator::Log2 => return self.logarithm_to_base(&values, 2.0),
            Operator::Log10 => return self.logarithm_to_base(&values, 10.0),
            Operator::Log => return self.logarithm_of_values(&values),
            Operator::Complex => return self.complex(arguments, &values),
            Operator::ToF32 => return self.conversion(expression, Domain::F32, &values),
            Operator::ToF64 => return self.conversion(expression, Domain::F64, &values),
            _ => {}
        }
        if values
            .iter()
            .any(|value| matches!(value, Value::Complex { .. }))
        {
            return self.complex_operation(operator, &values);
        }
        let operation =
            plan_operation(operator).ok_or(LowerError::UnsupportedOperator(operator))?;
        let signature = operation.signature();
        let gates = signature
            .arguments
            .iter()
            .zip(arguments.iter().zip(&values))
            .map(|(kind, (argument, value))| match (kind, value) {
                (ValueKind::Scalar, Value::Scalar(gate)) => Ok(*gate),
                (ValueKind::Boolean, Value::Boolean(gate)) => Ok(*gate),
                (ValueKind::Scalar, _) => Err(LowerError::ScalarExpected(*argument)),
                (ValueKind::Boolean, _) => Err(LowerError::BooleanExpected(*argument)),
            })
            .collect::<Result<Vec<usize>, LowerError>>()?;
        let gate = self.operation(operation, &gates);
        Ok(match signature.result {
            ValueKind::Scalar => Value::Scalar(gate),
            ValueKind::Boolean => Value::Boolean(gate),
        })
    }

    fn complex(&mut self, arguments: &[ExprId], values: &[Value]) -> Result<Value, LowerError> {
        if let [Value::Scalar(real), Value::Scalar(imaginary)] = values {
            return Ok(Value::Complex {
                real: *real,
                imaginary: *imaginary,
            });
        }
        let offending = arguments
            .iter()
            .zip(values)
            .find(|(_, value)| !matches!(value, Value::Scalar(_)))
            .map(|(argument, _)| *argument);
        match offending {
            Some(argument) => Err(LowerError::ScalarExpected(argument)),
            None => Err(LowerError::UnsupportedOperator(Operator::Complex)),
        }
    }

    fn conversion(
        &mut self,
        expression: ExprId,
        target: Domain,
        values: &[Value],
    ) -> Result<Value, LowerError> {
        match values {
            [value] if target == self.domain => Ok(*value),
            [_] => Err(LowerError::MixedMachineDomain {
                expression,
                found: target,
            }),
            _ => Err(LowerError::UnsupportedOperator(match target {
                Domain::F32 => Operator::ToF32,
                Domain::F64 => Operator::ToF64,
            })),
        }
    }

    fn as_complex(
        &mut self,
        value: Value,
        operator: Operator,
    ) -> Result<(usize, usize), LowerError> {
        match value {
            Value::Complex { real, imaginary } => Ok((real, imaginary)),
            Value::Scalar(real) => Ok((real, self.constant_f32_or_f64(0.0, 0.0))),
            Value::Boolean(_) => Err(LowerError::ComplexNotSupported(operator)),
        }
    }

    fn complex_operation(
        &mut self,
        operator: Operator,
        values: &[Value],
    ) -> Result<Value, LowerError> {
        match (operator, values) {
            (Operator::Neg, [value]) => {
                let (real, imaginary) = self.as_complex(*value, operator)?;
                Ok(Value::Complex {
                    real: self.operation(PlanOp::Neg, &[real]),
                    imaginary: self.operation(PlanOp::Neg, &[imaginary]),
                })
            }
            (Operator::Add | Operator::Sub, [left, right]) => {
                let operation = if operator == Operator::Add {
                    PlanOp::Add
                } else {
                    PlanOp::Sub
                };
                let (left_real, left_imaginary) = self.as_complex(*left, operator)?;
                let (right_real, right_imaginary) = self.as_complex(*right, operator)?;
                Ok(Value::Complex {
                    real: self.operation(operation, &[left_real, right_real]),
                    imaginary: self.operation(operation, &[left_imaginary, right_imaginary]),
                })
            }
            (Operator::Mul, [left, right]) => {
                let (a, b) = self.as_complex(*left, operator)?;
                let (c, d) = self.as_complex(*right, operator)?;
                let real_product = self.operation(PlanOp::Mul, &[a, c]);
                let imaginary_product = self.operation(PlanOp::Mul, &[b, d]);
                let real = self.operation(PlanOp::Sub, &[real_product, imaginary_product]);
                let first_cross = self.operation(PlanOp::Mul, &[a, d]);
                let second_cross = self.operation(PlanOp::Mul, &[b, c]);
                let imaginary = self.operation(PlanOp::Add, &[first_cross, second_cross]);
                Ok(Value::Complex { real, imaginary })
            }
            _ => Err(LowerError::ComplexNotSupported(operator)),
        }
    }

    fn finish(self, outputs: Vec<usize>, reduce: Option<Reduce>) -> Result<Plan, LowerError> {
        let channel_count = self.channel_count();
        Plan::new(
            self.domain,
            channel_count,
            self.gates,
            outputs.into_iter().map(gate_id).collect(),
            reduce,
        )
        .map_err(LowerError::Plan)
    }
}

fn constant_integer(pool: &ExprPool, expression: ExprId) -> Option<calc_numbers::Integer> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => match pool.number_value(number).ok()? {
            Number::Integer(integer) => Some(integer.clone()),
            _ => None,
        },
        _ => None,
    }
}

pub fn lower(pool: &ExprPool, root: ExprId, spec: &LowerSpec) -> Result<Plan, LowerError> {
    let mut lowering = Lowering::new(pool, spec)?;
    match pool.node(root).map_err(LowerError::Access)? {
        NodeView::Bind {
            binder: binder @ (BinderKind::Sum(shape) | BinderKind::Product(shape)),
            arguments,
            body,
        } => {
            let (lower_bound, upper_bound) = match arguments {
                [lower_bound, upper_bound] => (
                    constant_integer(pool, *lower_bound),
                    constant_integer(pool, *upper_bound),
                ),
                _ => (None, None),
            };
            let (Some(lower_bound), Some(upper_bound)) = (lower_bound, upper_bound) else {
                return Err(LowerError::NonConstantBounds(root));
            };
            if upper_bound < lower_bound {
                return Err(LowerError::EmptyRange(root));
            }
            if spec.output == ValueForm::Complex {
                return Err(LowerError::ComplexReduction(root));
            }
            let channel = lowering.channel_count();
            lowering.bound = Some(lowering.push(Gate::Input(channel_id(channel))));
            let output = match lowering.value(body)? {
                Value::Scalar(gate) => gate,
                Value::Complex { .. } => return Err(LowerError::ComplexReduction(root)),
                Value::Boolean(_) => return Err(LowerError::ScalarExpected(body)),
            };
            let operation = match binder {
                BinderKind::Product(_) => ReduceOperation::Mul,
                _ => ReduceOperation::Add,
            };
            lowering.finish(
                vec![output],
                Some(Reduce {
                    operation,
                    shape: reduce_shape(shape),
                }),
            )
        }
        _ => {
            let outputs = match (spec.output, lowering.value(root)?) {
                (_, Value::Boolean(_)) => return Err(LowerError::ScalarExpected(root)),
                (ValueForm::Real, Value::Scalar(gate)) => vec![gate],
                (ValueForm::Complex, Value::Scalar(gate)) => {
                    vec![gate, lowering.constant_f32_or_f64(0.0, 0.0)]
                }
                (ValueForm::Complex, Value::Complex { real, imaginary }) => vec![real, imaginary],
                (ValueForm::Real, Value::Complex { .. }) => {
                    return Err(LowerError::OutputFormMismatch {
                        requested: ValueForm::Real,
                        found: ValueForm::Complex,
                    });
                }
            };
            lowering.finish(outputs, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::SymbolKind;
    use calc_numbers::Integer;

    fn variable(pool: &mut ExprPool, name: &str) -> (SymbolId, ExprId) {
        let symbol = pool.intern_symbol(name, SymbolKind::Variable).unwrap();
        (symbol, pool.symbol(symbol).unwrap())
    }

    fn integer(pool: &mut ExprPool, value: i64) -> ExprId {
        pool.number(Number::Integer(Integer::from(value))).unwrap()
    }

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn real_spec(domain: Domain, symbols: &[SymbolId]) -> LowerSpec {
        LowerSpec {
            domain,
            inputs: symbols
                .iter()
                .map(|symbol| LowerInput {
                    symbol: *symbol,
                    form: ValueForm::Real,
                })
                .collect(),
            output: ValueForm::Real,
        }
    }

    fn operation_at(plan: &Plan, gate: usize) -> Option<(PlanOp, Vec<u32>)> {
        match plan.gates().get(gate) {
            Some(Gate::Operation {
                operation,
                arguments,
            }) => Some((
                *operation,
                arguments.iter().map(|argument| argument.0).collect(),
            )),
            _ => None,
        }
    }

    fn constant_f64_bits(plan: &Plan, gate: usize) -> Option<u64> {
        match plan.gates().get(gate) {
            Some(Gate::Constant(Constant::F64(value))) => Some(value.to_bits()),
            _ => None,
        }
    }

    #[test]
    fn exact_decimal_is_rounded_once_to_f64_at_the_leaf() {
        let mut pool = ExprPool::new();
        let tenth = pool
            .number(Number::fraction(&Integer::from(1_i64), &Integer::from(10_i64)).unwrap())
            .unwrap();

        let plan = lower(&pool, tenth, &real_spec(Domain::F64, &[])).unwrap();

        assert_eq!(constant_f64_bits(&plan, 0), Some(0.1_f64.to_bits()));
    }

    #[test]
    fn exact_decimal_is_rounded_once_to_f32_at_the_leaf() {
        let mut pool = ExprPool::new();
        let tenth = pool
            .number(Number::fraction(&Integer::from(1_i64), &Integer::from(10_i64)).unwrap())
            .unwrap();

        let plan = lower(&pool, tenth, &real_spec(Domain::F32, &[])).unwrap();

        assert!(matches!(
            plan.gates().first(),
            Some(Gate::Constant(Constant::F32(value))) if value.to_bits() == 0.1_f32.to_bits()
        ));
    }

    #[test]
    fn machine_number_of_the_other_domain_is_rejected() {
        let mut pool = ExprPool::new();
        let single = pool.number(Number::F32(1.5)).unwrap();

        let result = lower(&pool, single, &real_spec(Domain::F64, &[]));

        assert_eq!(
            result.err(),
            Some(LowerError::MixedMachineDomain {
                expression: single,
                found: Domain::F32
            })
        );
    }

    #[test]
    fn input_symbol_becomes_its_channel() {
        let mut pool = ExprPool::new();
        let (x, _) = variable(&mut pool, "x");
        let (y, y_node) = variable(&mut pool, "y");

        let plan = lower(&pool, y_node, &real_spec(Domain::F64, &[x, y])).unwrap();

        assert!(matches!(
            plan.gates()[plan.outputs()[0].index()],
            Gate::Input(ChannelId(1))
        ));
    }

    #[test]
    fn complex_input_takes_two_channels() {
        let mut pool = ExprPool::new();
        let (z, z_node) = variable(&mut pool, "z");
        let spec = LowerSpec {
            domain: Domain::F64,
            inputs: vec![LowerInput {
                symbol: z,
                form: ValueForm::Complex,
            }],
            output: ValueForm::Complex,
        };

        let plan = lower(&pool, z_node, &spec).unwrap();

        assert_eq!(
            (plan.input_channel_count(), plan.output_channel_count()),
            (2, 2)
        );
    }

    #[test]
    fn builtin_pi_is_the_nearest_machine_value() {
        let mut pool = ExprPool::new();
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();

        let plan = lower(&pool, pi, &real_spec(Domain::F64, &[])).unwrap();

        assert_eq!(
            constant_f64_bits(&plan, 0),
            Some(std::f64::consts::PI.to_bits())
        );
    }

    #[test]
    fn symbol_without_input_is_rejected() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");

        let result = lower(&pool, x_node, &real_spec(Domain::F64, &[]));

        assert_eq!(result.err(), Some(LowerError::UnboundSymbol(x)));
    }

    #[test]
    fn duplicate_input_is_rejected() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");

        let result = lower(&pool, x_node, &real_spec(Domain::F64, &[x, x]));

        assert_eq!(result.err(), Some(LowerError::DuplicateInput(x)));
    }

    #[test]
    fn bound_variable_outside_a_sum_is_rejected() {
        let mut pool = ExprPool::new();
        let bound = pool.bound(0).unwrap();

        let result = lower(&pool, bound, &real_spec(Domain::F64, &[]));

        assert_eq!(result.err(), Some(LowerError::UnexpectedBound(0)));
    }

    #[test]
    fn arithmetic_keeps_the_written_shape() {
        let mut pool = ExprPool::new();
        let (a, a_node) = variable(&mut pool, "a");
        let (b, b_node) = variable(&mut pool, "b");
        let (c, c_node) = variable(&mut pool, "c");
        let inner = apply(&mut pool, Operator::Add, &[a_node, b_node]);
        let outer = apply(&mut pool, Operator::Add, &[inner, c_node]);

        let plan = lower(&pool, outer, &real_spec(Domain::F64, &[a, b, c])).unwrap();

        assert_eq!(
            (operation_at(&plan, 3), operation_at(&plan, 4)),
            (
                Some((PlanOp::Add, vec![0, 1])),
                Some((PlanOp::Add, vec![3, 2]))
            )
        );
    }

    #[test]
    fn shared_subexpression_becomes_one_gate() {
        let mut pool = ExprPool::new();
        let (a, a_node) = variable(&mut pool, "a");
        let (b, b_node) = variable(&mut pool, "b");
        let sum = apply(&mut pool, Operator::Add, &[a_node, b_node]);
        let square = apply(&mut pool, Operator::Mul, &[sum, sum]);

        let plan = lower(&pool, square, &real_spec(Domain::F64, &[a, b])).unwrap();

        assert_eq!(operation_at(&plan, 3), Some((PlanOp::Mul, vec![2, 2])));
    }

    #[test]
    fn elementary_function_maps_to_its_plan_operation() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");
        let sine = apply(&mut pool, Operator::Sin, &[x_node]);

        let plan = lower(&pool, sine, &real_spec(Domain::F64, &[x])).unwrap();

        assert_eq!(operation_at(&plan, 1), Some((PlanOp::Sin, vec![0])));
    }

    #[test]
    fn inverse_sine_becomes_its_operation() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");
        let arcsine = apply(&mut pool, Operator::Asin, &[x_node]);

        let plan = lower(&pool, arcsine, &real_spec(Domain::F64, &[x])).unwrap();

        assert_eq!(operation_at(&plan, 1), Some((PlanOp::Asin, vec![0])));
    }

    #[test]
    fn factorial_is_unsupported() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");
        let factorial = apply(&mut pool, Operator::Factorial, &[x_node]);

        let result = lower(&pool, factorial, &real_spec(Domain::F64, &[x]));

        assert_eq!(
            result.err(),
            Some(LowerError::UnsupportedOperator(Operator::Factorial))
        );
    }

    #[test]
    fn comparison_feeds_select() {
        let mut pool = ExprPool::new();
        let (a, a_node) = variable(&mut pool, "a");
        let (b, b_node) = variable(&mut pool, "b");
        let less = apply(&mut pool, Operator::Less, &[a_node, b_node]);
        let minimum = apply(&mut pool, Operator::Select, &[less, a_node, b_node]);

        let plan = lower(&pool, minimum, &real_spec(Domain::F64, &[a, b])).unwrap();

        assert_eq!(
            operation_at(&plan, 3),
            Some((PlanOp::Select, vec![2, 0, 1]))
        );
    }

    #[test]
    fn comparison_as_the_result_is_rejected() {
        let mut pool = ExprPool::new();
        let (a, a_node) = variable(&mut pool, "a");
        let less = apply(&mut pool, Operator::Less, &[a_node, a_node]);

        let result = lower(&pool, less, &real_spec(Domain::F64, &[a]));

        assert_eq!(result.err(), Some(LowerError::ScalarExpected(less)));
    }

    #[test]
    fn logic_on_scalars_is_rejected() {
        let mut pool = ExprPool::new();
        let (a, a_node) = variable(&mut pool, "a");
        let conjunction = apply(&mut pool, Operator::And, &[a_node, a_node]);

        let result = lower(&pool, conjunction, &real_spec(Domain::F64, &[a]));

        assert_eq!(result.err(), Some(LowerError::BooleanExpected(a_node)));
    }

    #[test]
    fn complex_product_expands_into_real_gates() {
        let mut pool = ExprPool::new();
        let (a, a_node) = variable(&mut pool, "a");
        let (b, b_node) = variable(&mut pool, "b");
        let number = apply(&mut pool, Operator::Complex, &[a_node, b_node]);
        let unit = pool
            .symbol(BuiltinConstant::ImaginaryUnit.symbol())
            .unwrap();
        let product = apply(&mut pool, Operator::Mul, &[number, unit]);
        let mut spec = real_spec(Domain::F64, &[a, b]);
        spec.output = ValueForm::Complex;

        let plan = lower(&pool, product, &spec).unwrap();

        assert_eq!(
            plan.outputs()
                .iter()
                .map(|output| operation_at(&plan, output.index()).map(|(operation, _)| operation))
                .collect::<Vec<_>>(),
            vec![Some(PlanOp::Sub), Some(PlanOp::Add)]
        );
    }

    #[test]
    fn complex_result_with_real_output_is_rejected() {
        let mut pool = ExprPool::new();
        let unit = pool
            .symbol(BuiltinConstant::ImaginaryUnit.symbol())
            .unwrap();

        let result = lower(&pool, unit, &real_spec(Domain::F64, &[]));

        assert_eq!(
            result.err(),
            Some(LowerError::OutputFormMismatch {
                requested: ValueForm::Real,
                found: ValueForm::Complex
            })
        );
    }

    #[test]
    fn complex_division_is_unsupported() {
        let mut pool = ExprPool::new();
        let unit = pool
            .symbol(BuiltinConstant::ImaginaryUnit.symbol())
            .unwrap();
        let quotient = apply(&mut pool, Operator::Div, &[unit, unit]);
        let mut spec = real_spec(Domain::F64, &[]);
        spec.output = ValueForm::Complex;

        let result = lower(&pool, quotient, &spec);

        assert_eq!(
            result.err(),
            Some(LowerError::ComplexNotSupported(Operator::Div))
        );
    }

    #[test]
    fn conversion_to_the_plan_domain_adds_no_gate() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");
        let converted = apply(&mut pool, Operator::ToF64, &[x_node]);

        let plan = lower(&pool, converted, &real_spec(Domain::F64, &[x])).unwrap();

        assert_eq!(plan.gate_count(), 1);
    }

    #[test]
    fn conversion_to_the_other_domain_is_rejected() {
        let mut pool = ExprPool::new();
        let (x, x_node) = variable(&mut pool, "x");
        let converted = apply(&mut pool, Operator::ToF32, &[x_node]);

        let result = lower(&pool, converted, &real_spec(Domain::F64, &[x]));

        assert_eq!(
            result.err(),
            Some(LowerError::MixedMachineDomain {
                expression: converted,
                found: Domain::F32
            })
        );
    }

    #[test]
    fn measurement_uncertainty_operator_is_unsupported() {
        let mut pool = ExprPool::new();
        let value = integer(&mut pool, 3);
        let spread = integer(&mut pool, 1);
        let mark = pool.measurement().unwrap();
        let uncertain = apply(&mut pool, Operator::Uncertain, &[value, spread, mark]);

        let result = lower(&pool, uncertain, &real_spec(Domain::F64, &[]));

        assert_eq!(
            result.err(),
            Some(LowerError::UnsupportedOperator(Operator::Uncertain))
        );
    }

    #[test]
    fn user_function_is_rejected() {
        let mut pool = ExprPool::new();
        let function = pool
            .intern_symbol("f", SymbolKind::Function { arity: 1 })
            .unwrap();
        let argument = integer(&mut pool, 2);
        let call = pool.apply(Head::Function(function), &[argument]).unwrap();

        let result = lower(&pool, call, &real_spec(Domain::F64, &[]));

        assert_eq!(
            result.err(),
            Some(LowerError::UnsupportedFunction(function))
        );
    }

    fn summation(
        pool: &mut ExprPool,
        binder: BinderKind,
        lower_bound: i64,
        upper_bound: i64,
    ) -> (SymbolId, ExprId) {
        let (x, x_node) = variable(pool, "x");
        let first = integer(pool, lower_bound);
        let last = integer(pool, upper_bound);
        let index = pool.bound(0).unwrap();
        let body = apply(pool, Operator::Mul, &[index, x_node]);
        (x, pool.bind(binder, &[first, last], body).unwrap())
    }

    #[test]
    fn sum_becomes_an_add_reduce_with_the_written_shape() {
        let mut pool = ExprPool::new();
        let (x, sum) = summation(&mut pool, BinderKind::Sum(ReductionShape::Halving), 1, 10);

        let plan = lower(&pool, sum, &real_spec(Domain::F64, &[x])).unwrap();

        assert_eq!(
            plan.reduce(),
            Some(Reduce {
                operation: ReduceOperation::Add,
                shape: ReduceShape::Halving
            })
        );
    }

    #[test]
    fn sum_index_is_the_channel_after_the_inputs() {
        let mut pool = ExprPool::new();
        let (x, sum) = summation(&mut pool, BinderKind::Sum(ReductionShape::LeftFold), 1, 10);

        let plan = lower(&pool, sum, &real_spec(Domain::F64, &[x])).unwrap();

        assert!(matches!(plan.gates()[1], Gate::Input(ChannelId(1))));
    }

    #[test]
    fn product_becomes_a_mul_reduce() {
        let mut pool = ExprPool::new();
        let (x, product) = summation(
            &mut pool,
            BinderKind::Product(ReductionShape::LeftFold),
            1,
            3,
        );

        let plan = lower(&pool, product, &real_spec(Domain::F64, &[x])).unwrap();

        assert_eq!(
            plan.reduce().map(|reduce| reduce.operation),
            Some(ReduceOperation::Mul)
        );
    }

    #[test]
    fn sum_with_symbolic_bound_is_rejected() {
        let mut pool = ExprPool::new();
        let (n, n_node) = variable(&mut pool, "n");
        let first = integer(&mut pool, 1);
        let index = pool.bound(0).unwrap();
        let sum = pool
            .bind(
                BinderKind::Sum(ReductionShape::LeftFold),
                &[first, n_node],
                index,
            )
            .unwrap();

        let result = lower(&pool, sum, &real_spec(Domain::F64, &[n]));

        assert_eq!(result.err(), Some(LowerError::NonConstantBounds(sum)));
    }

    #[test]
    fn sum_over_empty_range_is_rejected() {
        let mut pool = ExprPool::new();
        let (x, sum) = summation(&mut pool, BinderKind::Sum(ReductionShape::LeftFold), 5, 4);

        let result = lower(&pool, sum, &real_spec(Domain::F64, &[x]));

        assert_eq!(result.err(), Some(LowerError::EmptyRange(sum)));
    }

    #[test]
    fn sum_of_complex_terms_is_rejected() {
        let mut pool = ExprPool::new();
        let first = integer(&mut pool, 1);
        let last = integer(&mut pool, 2);
        let unit = pool
            .symbol(BuiltinConstant::ImaginaryUnit.symbol())
            .unwrap();
        let sum = pool
            .bind(
                BinderKind::Sum(ReductionShape::LeftFold),
                &[first, last],
                unit,
            )
            .unwrap();

        let result = lower(&pool, sum, &real_spec(Domain::F64, &[]));

        assert_eq!(result.err(), Some(LowerError::ComplexReduction(sum)));
    }

    #[test]
    fn integral_is_rejected() {
        let mut pool = ExprPool::new();
        let index = pool.bound(0).unwrap();
        let integral = pool.bind(BinderKind::Integral, &[], index).unwrap();

        let result = lower(&pool, integral, &real_spec(Domain::F64, &[]));

        assert_eq!(
            result.err(),
            Some(LowerError::UnsupportedBinder(BinderKind::Integral))
        );
    }

    #[test]
    fn quantity_is_rejected() {
        let mut pool = ExprPool::new();
        let value = integer(&mut pool, 3);
        let metre = pool.units_mut().lookup("m").unwrap();
        let length = pool.quantity(value, metre).unwrap();

        let result = lower(&pool, length, &real_spec(Domain::F64, &[]));

        assert_eq!(result.err(), Some(LowerError::QuantityNotConverted(length)));
    }

    #[test]
    fn array_is_rejected() {
        let mut pool = ExprPool::new();
        let element = integer(&mut pool, 1);
        let vector = pool.array(&[2], &[element, element]).unwrap();

        let result = lower(&pool, vector, &real_spec(Domain::F64, &[]));

        assert_eq!(result.err(), Some(LowerError::ArrayNotSupported(vector)));
    }

    #[test]
    fn expression_from_another_pool_is_an_access_error() {
        let mut larger = ExprPool::new();
        integer(&mut larger, 1);
        let foreign = integer(&mut larger, 2);
        let pool = ExprPool::new();

        let result = lower(&pool, foreign, &real_spec(Domain::F64, &[]));

        assert_eq!(
            result.err(),
            Some(LowerError::Access(AccessError::UnknownExprId(foreign)))
        );
    }
}
