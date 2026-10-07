use std::collections::HashMap;

use calc_exec::{
    BodyGate, Constant, Gate, Iteration, Plan, PlanOp, PrepareError, ReduceShape, Unsupported,
};

use crate::kernel::{
    BinaryOperation, BufferKind, CARRY, CONSTANTS, Comparison, Expression, INPUTS, Kernel,
    KernelIteration, LocalId, MapPass, OUTPUTS, REDUCE_TARGET, ReducePass, TRIPLES, UnaryOperation,
};

fn local(gate: usize) -> LocalId {
    LocalId(u32::try_from(gate).unwrap_or(u32::MAX))
}

fn unsupported_operation(operation: PlanOp) -> PrepareError {
    PrepareError::Unsupported(Unsupported::Operation(operation))
}

fn expression(operation: PlanOp, arguments: &[LocalId]) -> Result<Expression, PrepareError> {
    let expression = match (operation, arguments) {
        (PlanOp::Neg, &[value]) => Expression::Unary(UnaryOperation::Neg, value),
        (PlanOp::Abs, &[value]) => Expression::Unary(UnaryOperation::Abs, value),
        (PlanOp::Sqrt, &[value]) => Expression::Unary(UnaryOperation::Sqrt, value),
        (PlanOp::Floor, &[value]) => Expression::Unary(UnaryOperation::Floor, value),
        (PlanOp::Ceil, &[value]) => Expression::Unary(UnaryOperation::Ceil, value),
        (PlanOp::Trunc, &[value]) => Expression::Unary(UnaryOperation::Trunc, value),
        (PlanOp::RoundTiesEven, &[value]) => {
            Expression::Unary(UnaryOperation::RoundTiesEven, value)
        }
        (PlanOp::Add, &[left, right]) => Expression::Binary(BinaryOperation::Add, left, right),
        (PlanOp::Sub, &[left, right]) => Expression::Binary(BinaryOperation::Sub, left, right),
        (PlanOp::Mul, &[left, right]) => Expression::Binary(BinaryOperation::Mul, left, right),
        (PlanOp::Div, &[left, right]) => Expression::Binary(BinaryOperation::Div, left, right),
        (PlanOp::Min, &[left, right]) => Expression::Binary(BinaryOperation::Min, left, right),
        (PlanOp::Max, &[left, right]) => Expression::Binary(BinaryOperation::Max, left, right),
        (PlanOp::CopySign, &[left, right]) => {
            Expression::Binary(BinaryOperation::CopySign, left, right)
        }
        (PlanOp::Less, &[left, right]) => Expression::Compare(Comparison::Less, left, right),
        (PlanOp::LessOrEqual, &[left, right]) => {
            Expression::Compare(Comparison::LessOrEqual, left, right)
        }
        (PlanOp::Greater, &[left, right]) => Expression::Compare(Comparison::Greater, left, right),
        (PlanOp::GreaterOrEqual, &[left, right]) => {
            Expression::Compare(Comparison::GreaterOrEqual, left, right)
        }
        (PlanOp::Equal, &[left, right]) => Expression::Compare(Comparison::Equal, left, right),
        (PlanOp::NotEqual, &[left, right]) => {
            Expression::Compare(Comparison::NotEqual, left, right)
        }
        (PlanOp::And, &[left, right]) => Expression::And(left, right),
        (PlanOp::Or, &[left, right]) => Expression::Or(left, right),
        (PlanOp::Not, &[value]) => Expression::Not(value),
        (PlanOp::Select, &[condition, when_true, when_false]) => Expression::Select {
            condition,
            when_true,
            when_false,
        },
        _ => return Err(unsupported_operation(operation)),
    };
    Ok(expression)
}

fn constant_index(
    constants: &mut Vec<f32>,
    constant: Constant,
) -> Result<Expression, PrepareError> {
    match constant {
        Constant::F32(value) => {
            constants.push(value);
            Ok(Expression::Constant {
                index: u32::try_from(constants.len() - 1).unwrap_or(u32::MAX),
            })
        }
        Constant::F64(_) => Err(PrepareError::Unsupported(Unsupported::Domain(
            calc_exec::Domain::F64,
        ))),
    }
}

fn lower_iteration(
    iteration: &Iteration,
    constants: &mut Vec<f32>,
) -> Result<KernelIteration, PrepareError> {
    let mut body = Vec::with_capacity(iteration.body.len());
    for body_gate in &iteration.body {
        let expression = match body_gate {
            BodyGate::State(slot) => Expression::State { slot: slot.0 },
            BodyGate::Outer(gate) => Expression::Outer {
                local: local(gate.index()),
            },
            BodyGate::Constant(constant) => constant_index(constants, *constant)?,
            BodyGate::Operation {
                operation,
                arguments,
            } => {
                let arguments: Vec<LocalId> = arguments
                    .iter()
                    .map(|argument| local(argument.index()))
                    .collect();
                expression(*operation, &arguments)?
            }
        };
        body.push(expression);
    }
    Ok(KernelIteration {
        initial: iteration
            .initial
            .iter()
            .map(|gate| local(gate.index()))
            .collect(),
        body,
        next: iteration
            .next
            .iter()
            .map(|next| local(next.index()))
            .collect(),
        exit: local(iteration.exit.index()),
        maximum_count: iteration.maximum_count,
    })
}

pub fn lower_plan(plan: &Plan) -> Result<Kernel, PrepareError> {
    let mut statements = Vec::with_capacity(plan.gate_count());
    let mut constants = Vec::new();
    let mut iterations = Vec::new();
    for gate in plan.gates() {
        let statement = match gate {
            Gate::Input(channel) => Expression::Input { channel: channel.0 },
            Gate::Constant(constant) => constant_index(&mut constants, *constant)?,
            Gate::Operation {
                operation,
                arguments,
            } => {
                let arguments: Vec<LocalId> = arguments
                    .iter()
                    .map(|argument| local(argument.index()))
                    .collect();
                expression(*operation, &arguments)?
            }
            Gate::Iterate(iteration) => {
                iterations.push(lower_iteration(iteration, &mut constants)?);
                Expression::Iterate {
                    iteration: u32::try_from(iterations.len() - 1).unwrap_or(u32::MAX),
                }
            }
            Gate::IterationState { iteration, slot } => Expression::IterationState {
                iteration: local(iteration.index()),
                slot: slot.0,
            },
        };
        statements.push(statement);
    }
    let reduce = plan
        .reduce()
        .filter(|reduce| reduce.shape == ReduceShape::Halving)
        .map(|reduce| ReducePass {
            operation: reduce.operation,
            source: OUTPUTS,
            target: REDUCE_TARGET,
            triples: TRIPLES,
        });
    Ok(Kernel {
        buffers: vec![
            BufferKind::Values,
            BufferKind::Values,
            BufferKind::Values,
            BufferKind::Values,
            BufferKind::Indices,
        ],
        constants,
        map: MapPass {
            inputs: INPUTS,
            constants: CONSTANTS,
            outputs: OUTPUTS,
            input_channel_count: u32::try_from(plan.input_channel_count()).unwrap_or(u32::MAX),
            statements,
            results: plan
                .outputs()
                .iter()
                .map(|output| local(output.index()))
                .collect(),
            iterations,
        },
        reduce,
    })
}

enum Node {
    Leaf,
    Join(usize, usize),
}

struct Tree {
    nodes: Vec<Node>,
    heights: Vec<u32>,
}

impl Tree {
    fn build(&mut self, length: u32) -> usize {
        if length <= 1 {
            self.nodes.push(Node::Leaf);
            self.heights.push(0);
            return self.nodes.len() - 1;
        }
        let half = length / 2;
        let left = self.build(half);
        let right = self.build(length - half);
        let height = self.heights[left].max(self.heights[right]) + 1;
        self.nodes.push(Node::Join(left, right));
        self.heights.push(height);
        self.nodes.len() - 1
    }

    fn items(&self, node: usize, stage: u32, into: &mut Vec<usize>) {
        match self.nodes[node] {
            Node::Join(left, right) if self.heights[node] > stage => {
                self.items(left, stage, into);
                self.items(right, stage, into);
            }
            _ => into.push(node),
        }
    }
}

pub fn halving_stages(length: u32) -> Vec<Vec<u32>> {
    if length <= 1 {
        return Vec::new();
    }
    let mut tree = Tree {
        nodes: Vec::new(),
        heights: Vec::new(),
    };
    let root = tree.build(length);
    let mut previous = Vec::new();
    tree.items(root, 0, &mut previous);
    let mut stages = Vec::new();
    for stage in 1..=tree.heights[root] {
        let positions: HashMap<usize, u32> = previous
            .iter()
            .enumerate()
            .map(|(position, node)| (*node, u32::try_from(position).unwrap_or(CARRY)))
            .collect();
        let position_of = |node: usize| positions.get(&node).copied().unwrap_or(CARRY);
        let mut current = Vec::new();
        tree.items(root, stage, &mut current);
        let mut triples = Vec::with_capacity(current.len() * 3);
        for (result, node) in current.iter().enumerate() {
            let result = u32::try_from(result).unwrap_or(CARRY);
            match tree.nodes[*node] {
                Node::Join(left, right) if tree.heights[*node] == stage => {
                    triples.extend([position_of(left), position_of(right), result]);
                }
                _ => triples.extend([position_of(*node), CARRY, result]),
            }
        }
        stages.push(triples);
        previous = current;
    }
    stages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_element_needs_no_stage() {
        let stages = halving_stages(1);

        assert!(stages.is_empty());
    }

    #[test]
    fn three_elements_carry_the_first_into_the_second_stage() {
        let stages = halving_stages(3);

        assert_eq!(stages, vec![vec![0, CARRY, 0, 1, 2, 1], vec![0, 1, 0]]);
    }

    #[test]
    fn stage_count_is_the_tree_depth() {
        let stages = halving_stages(1025);

        assert_eq!(stages.len(), 11);
    }

    #[test]
    fn last_stage_has_one_result() {
        let stages = halving_stages(1024);

        assert_eq!(stages.last().map(Vec::len), Some(3));
    }
}
