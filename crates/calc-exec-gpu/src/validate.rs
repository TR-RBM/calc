use crate::kernel::{BufferId, BufferKind, Expression, Kernel, KernelIteration, LocalId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KernelError {
    BufferOutOfRange(BufferId),
    BufferKindMismatch {
        buffer: BufferId,
        expected: BufferKind,
    },
    ReadWriteSameBuffer(BufferId),
    Statement {
        iteration: Option<u32>,
        statement: usize,
        error: StatementError,
    },
    Iteration {
        iteration: u32,
        error: IterationError,
    },
    NoResults,
    ResultOutOfRange(LocalId),
    ResultNotValue(LocalId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatementError {
    InputChannelOutOfRange(u32),
    ConstantOutOfRange(u32),
    LocalNotEarlier(LocalId),
    LocalKindMismatch { local: LocalId, expected: LocalKind },
    NoValue(LocalId),
    OutOfScope,
    IterationOutOfRange(u32),
    NotAnIteration(LocalId),
    SlotOutOfRange(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IterationError {
    InitialNotEarlier(LocalId),
    InitialNoValue(LocalId),
    SlotCountMismatch { initial: usize, next: usize },
    SlotKindMismatch(usize),
    NextOutOfRange(LocalId),
    ExitOutOfRange(LocalId),
    ExitNotTruth(LocalId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalKind {
    Value,
    Truth,
}

fn check_buffer(
    kernel: &Kernel,
    buffer: BufferId,
    expected: BufferKind,
) -> Result<(), KernelError> {
    match kernel.buffers.get(buffer.index()) {
        None => Err(KernelError::BufferOutOfRange(buffer)),
        Some(kind) if *kind != expected => {
            Err(KernelError::BufferKindMismatch { buffer, expected })
        }
        Some(_) => Ok(()),
    }
}

fn check_distinct(read: &[BufferId], written: BufferId) -> Result<(), KernelError> {
    if read.contains(&written) {
        return Err(KernelError::ReadWriteSameBuffer(written));
    }
    Ok(())
}

enum Scope<'scope> {
    Map {
        iteration_slots: &'scope [Option<Vec<LocalKind>>],
    },
    Body {
        slots: &'scope [LocalKind],
        outer: &'scope [Option<LocalKind>],
    },
}

fn operand(
    kinds: &[Option<LocalKind>],
    local: LocalId,
    expected: LocalKind,
) -> Result<(), StatementError> {
    match kinds.get(local.index()) {
        None => Err(StatementError::LocalNotEarlier(local)),
        Some(None) => Err(StatementError::NoValue(local)),
        Some(Some(kind)) if *kind != expected => {
            Err(StatementError::LocalKindMismatch { local, expected })
        }
        Some(Some(_)) => Ok(()),
    }
}

fn statement_kind(
    kernel: &Kernel,
    scope: &Scope<'_>,
    kinds: &[Option<LocalKind>],
    expression: &Expression,
) -> Result<Option<LocalKind>, StatementError> {
    let value = |local| operand(kinds, local, LocalKind::Value);
    let truth = |local| operand(kinds, local, LocalKind::Truth);
    let kind = match (*expression, scope) {
        (Expression::Input { channel }, Scope::Map { .. }) => {
            if channel >= kernel.map.input_channel_count {
                return Err(StatementError::InputChannelOutOfRange(channel));
            }
            LocalKind::Value
        }
        (Expression::Constant { index }, _) => {
            let in_range = usize::try_from(index).is_ok_and(|index| index < kernel.constants.len());
            if !in_range {
                return Err(StatementError::ConstantOutOfRange(index));
            }
            LocalKind::Value
        }
        (Expression::Unary(_, argument), _) => {
            value(argument)?;
            LocalKind::Value
        }
        (Expression::Binary(_, left, right), _) => {
            value(left)?;
            value(right)?;
            LocalKind::Value
        }
        (Expression::Compare(_, left, right), _) => {
            value(left)?;
            value(right)?;
            LocalKind::Truth
        }
        (Expression::And(left, right) | Expression::Or(left, right), _) => {
            truth(left)?;
            truth(right)?;
            LocalKind::Truth
        }
        (Expression::Not(argument), _) => {
            truth(argument)?;
            LocalKind::Truth
        }
        (
            Expression::Select {
                condition,
                when_true,
                when_false,
            },
            _,
        ) => {
            truth(condition)?;
            value(when_true)?;
            value(when_false)?;
            LocalKind::Value
        }
        (Expression::Iterate { iteration }, Scope::Map { .. }) => {
            let in_range =
                usize::try_from(iteration).is_ok_and(|index| index < kernel.map.iterations.len());
            if !in_range {
                return Err(StatementError::IterationOutOfRange(iteration));
            }
            return Ok(None);
        }
        (Expression::IterationState { iteration, slot }, Scope::Map { iteration_slots }) => {
            let Some(Some(slots)) = iteration_slots.get(iteration.index()) else {
                return Err(StatementError::NotAnIteration(iteration));
            };
            *usize::try_from(slot)
                .ok()
                .and_then(|slot| slots.get(slot))
                .ok_or(StatementError::SlotOutOfRange(slot))?
        }
        (Expression::State { slot }, Scope::Body { slots, .. }) => *usize::try_from(slot)
            .ok()
            .and_then(|slot| slots.get(slot))
            .ok_or(StatementError::SlotOutOfRange(slot))?,
        (Expression::Outer { local }, Scope::Body { outer, .. }) => {
            match outer.get(local.index()) {
                None => return Err(StatementError::LocalNotEarlier(local)),
                Some(None) => return Err(StatementError::NoValue(local)),
                Some(Some(kind)) => *kind,
            }
        }
        (
            Expression::Input { .. }
            | Expression::Iterate { .. }
            | Expression::IterationState { .. },
            Scope::Body { .. },
        )
        | (Expression::State { .. } | Expression::Outer { .. }, Scope::Map { .. }) => {
            return Err(StatementError::OutOfScope);
        }
    };
    Ok(Some(kind))
}

fn iteration_slots(
    kernel: &Kernel,
    number: u32,
    iteration: &KernelIteration,
    outer: &[Option<LocalKind>],
) -> Result<Vec<LocalKind>, KernelError> {
    let failure = |error| KernelError::Iteration {
        iteration: number,
        error,
    };
    let mut slots = Vec::with_capacity(iteration.initial.len());
    for initial in &iteration.initial {
        match outer.get(initial.index()) {
            None => return Err(failure(IterationError::InitialNotEarlier(*initial))),
            Some(None) => return Err(failure(IterationError::InitialNoValue(*initial))),
            Some(Some(kind)) => slots.push(*kind),
        }
    }
    if iteration.next.len() != slots.len() {
        return Err(failure(IterationError::SlotCountMismatch {
            initial: slots.len(),
            next: iteration.next.len(),
        }));
    }
    let scope = Scope::Body {
        slots: &slots,
        outer,
    };
    let mut kinds: Vec<Option<LocalKind>> = Vec::with_capacity(iteration.body.len());
    for (statement, expression) in iteration.body.iter().enumerate() {
        let kind = statement_kind(kernel, &scope, &kinds, expression).map_err(|error| {
            KernelError::Statement {
                iteration: Some(number),
                statement,
                error,
            }
        })?;
        kinds.push(kind);
    }
    for (slot, (next, expected)) in iteration.next.iter().zip(&slots).enumerate() {
        match kinds.get(next.index()) {
            Some(Some(kind)) if kind == expected => {}
            Some(Some(_)) => return Err(failure(IterationError::SlotKindMismatch(slot))),
            _ => return Err(failure(IterationError::NextOutOfRange(*next))),
        }
    }
    match kinds.get(iteration.exit.index()) {
        Some(Some(LocalKind::Truth)) => Ok(slots),
        Some(Some(LocalKind::Value)) => Err(failure(IterationError::ExitNotTruth(iteration.exit))),
        _ => Err(failure(IterationError::ExitOutOfRange(iteration.exit))),
    }
}

pub fn validate(kernel: &Kernel) -> Result<(), KernelError> {
    let map = &kernel.map;
    check_buffer(kernel, map.inputs, BufferKind::Values)?;
    check_buffer(kernel, map.constants, BufferKind::Values)?;
    check_buffer(kernel, map.outputs, BufferKind::Values)?;
    check_distinct(&[map.inputs, map.constants], map.outputs)?;
    let mut kinds: Vec<Option<LocalKind>> = Vec::with_capacity(map.statements.len());
    let mut slots_by_statement: Vec<Option<Vec<LocalKind>>> =
        Vec::with_capacity(map.statements.len());
    for (statement, expression) in map.statements.iter().enumerate() {
        let scope = Scope::Map {
            iteration_slots: &slots_by_statement,
        };
        let kind = statement_kind(kernel, &scope, &kinds, expression).map_err(|error| {
            KernelError::Statement {
                iteration: None,
                statement,
                error,
            }
        })?;
        let slots = match *expression {
            Expression::Iterate { iteration } => {
                let found = usize::try_from(iteration)
                    .ok()
                    .and_then(|index| map.iterations.get(index));
                match found {
                    Some(body) => Some(iteration_slots(kernel, iteration, body, &kinds)?),
                    None => None,
                }
            }
            _ => None,
        };
        kinds.push(kind);
        slots_by_statement.push(slots);
    }
    if map.results.is_empty() {
        return Err(KernelError::NoResults);
    }
    for result in &map.results {
        match kinds.get(result.index()) {
            None => return Err(KernelError::ResultOutOfRange(*result)),
            Some(Some(LocalKind::Value)) => {}
            Some(_) => return Err(KernelError::ResultNotValue(*result)),
        }
    }
    if let Some(reduce) = kernel.reduce {
        check_buffer(kernel, reduce.source, BufferKind::Values)?;
        check_buffer(kernel, reduce.target, BufferKind::Values)?;
        check_buffer(kernel, reduce.triples, BufferKind::Indices)?;
        check_distinct(&[reduce.source, reduce.triples], reduce.target)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{
        BinaryOperation, CONSTANTS, Comparison, INPUTS, MapPass, OUTPUTS, REDUCE_TARGET,
        ReducePass, TRIPLES,
    };
    use calc_exec::ReduceOperation;

    fn kernel(statements: Vec<Expression>, results: Vec<LocalId>) -> Kernel {
        Kernel {
            buffers: vec![
                BufferKind::Values,
                BufferKind::Values,
                BufferKind::Values,
                BufferKind::Values,
                BufferKind::Indices,
            ],
            constants: vec![2.0],
            map: MapPass {
                inputs: INPUTS,
                constants: CONSTANTS,
                outputs: OUTPUTS,
                input_channel_count: 1,
                statements,
                results,
                iterations: Vec::new(),
            },
            reduce: None,
        }
    }

    fn sum_kernel() -> Kernel {
        kernel(
            vec![
                Expression::Input { channel: 0 },
                Expression::Constant { index: 0 },
                Expression::Binary(BinaryOperation::Add, LocalId(0), LocalId(1)),
            ],
            vec![LocalId(2)],
        )
    }

    #[test]
    fn valid_kernel_passes() {
        let kernel = sum_kernel();

        assert_eq!(validate(&kernel), Ok(()));
    }

    #[test]
    fn missing_buffer_is_rejected() {
        let mut kernel = sum_kernel();
        kernel.map.outputs = BufferId(9);

        assert_eq!(
            validate(&kernel),
            Err(KernelError::BufferOutOfRange(BufferId(9)))
        );
    }

    #[test]
    fn index_buffer_used_for_values_is_rejected() {
        let mut kernel = sum_kernel();
        kernel.map.outputs = TRIPLES;

        assert_eq!(
            validate(&kernel),
            Err(KernelError::BufferKindMismatch {
                buffer: TRIPLES,
                expected: BufferKind::Values
            })
        );
    }

    #[test]
    fn map_writing_its_input_buffer_is_rejected() {
        let mut kernel = sum_kernel();
        kernel.map.outputs = INPUTS;

        assert_eq!(
            validate(&kernel),
            Err(KernelError::ReadWriteSameBuffer(INPUTS))
        );
    }

    #[test]
    fn reduce_writing_its_source_buffer_is_rejected() {
        let mut kernel = sum_kernel();
        kernel.reduce = Some(ReducePass {
            operation: ReduceOperation::Add,
            source: OUTPUTS,
            target: OUTPUTS,
            triples: TRIPLES,
        });

        assert_eq!(
            validate(&kernel),
            Err(KernelError::ReadWriteSameBuffer(OUTPUTS))
        );
    }

    #[test]
    fn reduce_with_value_triples_is_rejected() {
        let mut kernel = sum_kernel();
        kernel.reduce = Some(ReducePass {
            operation: ReduceOperation::Add,
            source: OUTPUTS,
            target: REDUCE_TARGET,
            triples: CONSTANTS,
        });

        assert_eq!(
            validate(&kernel),
            Err(KernelError::BufferKindMismatch {
                buffer: CONSTANTS,
                expected: BufferKind::Indices
            })
        );
    }

    #[test]
    fn input_channel_beyond_count_is_rejected() {
        let kernel = kernel(vec![Expression::Input { channel: 1 }], vec![LocalId(0)]);

        assert_eq!(
            validate(&kernel),
            Err(KernelError::Statement {
                iteration: None,
                statement: 0,
                error: StatementError::InputChannelOutOfRange(1)
            })
        );
    }

    #[test]
    fn constant_beyond_table_is_rejected() {
        let kernel = kernel(vec![Expression::Constant { index: 1 }], vec![LocalId(0)]);

        assert_eq!(
            validate(&kernel),
            Err(KernelError::Statement {
                iteration: None,
                statement: 0,
                error: StatementError::ConstantOutOfRange(1)
            })
        );
    }

    #[test]
    fn local_used_before_its_statement_is_rejected() {
        let kernel = kernel(
            vec![Expression::Unary(
                crate::kernel::UnaryOperation::Neg,
                LocalId(0),
            )],
            vec![LocalId(0)],
        );

        assert_eq!(
            validate(&kernel),
            Err(KernelError::Statement {
                iteration: None,
                statement: 0,
                error: StatementError::LocalNotEarlier(LocalId(0))
            })
        );
    }

    #[test]
    fn value_used_as_truth_is_rejected() {
        let kernel = kernel(
            vec![
                Expression::Input { channel: 0 },
                Expression::Not(LocalId(0)),
            ],
            vec![LocalId(0)],
        );

        assert_eq!(
            validate(&kernel),
            Err(KernelError::Statement {
                iteration: None,
                statement: 1,
                error: StatementError::LocalKindMismatch {
                    local: LocalId(0),
                    expected: LocalKind::Truth
                }
            })
        );
    }

    #[test]
    fn kernel_without_results_is_rejected() {
        let kernel = kernel(vec![Expression::Input { channel: 0 }], Vec::new());

        assert_eq!(validate(&kernel), Err(KernelError::NoResults));
    }

    #[test]
    fn result_beyond_statements_is_rejected() {
        let kernel = kernel(vec![Expression::Input { channel: 0 }], vec![LocalId(3)]);

        assert_eq!(
            validate(&kernel),
            Err(KernelError::ResultOutOfRange(LocalId(3)))
        );
    }

    #[test]
    fn truth_result_is_rejected() {
        let kernel = kernel(
            vec![
                Expression::Input { channel: 0 },
                Expression::Compare(Comparison::Less, LocalId(0), LocalId(0)),
            ],
            vec![LocalId(1)],
        );

        assert_eq!(
            validate(&kernel),
            Err(KernelError::ResultNotValue(LocalId(1)))
        );
    }

    fn iterating_kernel(iteration: KernelIteration, after: Vec<Expression>) -> Kernel {
        let mut statements = vec![
            Expression::Input { channel: 0 },
            Expression::Constant { index: 0 },
            Expression::Iterate { iteration: 0 },
            Expression::IterationState {
                iteration: LocalId(2),
                slot: 0,
            },
        ];
        statements.extend(after);
        let mut built = kernel(statements, vec![LocalId(3)]);
        built.map.iterations = vec![iteration];
        built
    }

    fn doubling() -> KernelIteration {
        KernelIteration {
            initial: vec![LocalId(0)],
            body: vec![
                Expression::State { slot: 0 },
                Expression::Outer { local: LocalId(1) },
                Expression::Binary(BinaryOperation::Mul, LocalId(0), LocalId(1)),
                Expression::Compare(Comparison::Greater, LocalId(0), LocalId(1)),
            ],
            next: vec![LocalId(2)],
            exit: LocalId(3),
            maximum_count: 8,
        }
    }

    fn iteration_error(kernel: &Kernel) -> Option<IterationError> {
        match validate(kernel) {
            Err(KernelError::Iteration { error, .. }) => Some(error),
            _ => None,
        }
    }

    fn statement_error(kernel: &Kernel) -> Option<(Option<u32>, StatementError)> {
        match validate(kernel) {
            Err(KernelError::Statement {
                iteration, error, ..
            }) => Some((iteration, error)),
            _ => None,
        }
    }

    #[test]
    fn valid_iteration_passes() {
        let kernel = iterating_kernel(doubling(), Vec::new());

        assert_eq!(validate(&kernel), Ok(()));
    }

    #[test]
    fn iterate_statement_used_as_operand_is_rejected() {
        let kernel = iterating_kernel(
            doubling(),
            vec![Expression::Unary(
                crate::kernel::UnaryOperation::Neg,
                LocalId(2),
            )],
        );

        assert_eq!(
            statement_error(&kernel),
            Some((None, StatementError::NoValue(LocalId(2))))
        );
    }

    #[test]
    fn state_outside_a_body_is_rejected() {
        let kernel = iterating_kernel(doubling(), vec![Expression::State { slot: 0 }]);

        assert_eq!(
            statement_error(&kernel),
            Some((None, StatementError::OutOfScope))
        );
    }

    #[test]
    fn input_inside_a_body_is_rejected() {
        let mut iteration = doubling();
        iteration.body[1] = Expression::Input { channel: 0 };

        let kernel = iterating_kernel(iteration, Vec::new());

        assert_eq!(
            statement_error(&kernel),
            Some((Some(0), StatementError::OutOfScope))
        );
    }

    #[test]
    fn missing_iteration_is_rejected() {
        let mut kernel = iterating_kernel(doubling(), Vec::new());
        kernel.map.statements[2] = Expression::Iterate { iteration: 3 };

        assert_eq!(
            statement_error(&kernel),
            Some((None, StatementError::IterationOutOfRange(3)))
        );
    }

    #[test]
    fn iteration_state_of_another_statement_is_rejected() {
        let mut kernel = iterating_kernel(doubling(), Vec::new());
        kernel.map.statements[3] = Expression::IterationState {
            iteration: LocalId(1),
            slot: 0,
        };

        assert_eq!(
            statement_error(&kernel),
            Some((None, StatementError::NotAnIteration(LocalId(1))))
        );
    }

    #[test]
    fn iteration_state_beyond_the_slots_is_rejected() {
        let mut kernel = iterating_kernel(doubling(), Vec::new());
        kernel.map.statements[3] = Expression::IterationState {
            iteration: LocalId(2),
            slot: 1,
        };

        assert_eq!(
            statement_error(&kernel),
            Some((None, StatementError::SlotOutOfRange(1)))
        );
    }

    #[test]
    fn initial_after_the_loop_is_rejected() {
        let mut iteration = doubling();
        iteration.initial = vec![LocalId(3)];

        let kernel = iterating_kernel(iteration, Vec::new());

        assert_eq!(
            iteration_error(&kernel),
            Some(IterationError::InitialNotEarlier(LocalId(3)))
        );
    }

    #[test]
    fn initial_without_value_is_rejected() {
        let mut kernel = iterating_kernel(doubling(), Vec::new());
        kernel
            .map
            .statements
            .insert(2, Expression::Iterate { iteration: 0 });
        kernel.map.statements[4] = Expression::IterationState {
            iteration: LocalId(2),
            slot: 0,
        };
        kernel.map.results = vec![LocalId(4)];
        let mut second = doubling();
        second.initial = vec![LocalId(2)];
        kernel.map.iterations.push(second);
        kernel.map.statements[3] = Expression::Iterate { iteration: 1 };

        assert_eq!(
            iteration_error(&kernel),
            Some(IterationError::InitialNoValue(LocalId(2)))
        );
    }

    #[test]
    fn slot_count_mismatch_is_rejected() {
        let mut iteration = doubling();
        iteration.next = vec![LocalId(2), LocalId(2)];

        let kernel = iterating_kernel(iteration, Vec::new());

        assert_eq!(
            iteration_error(&kernel),
            Some(IterationError::SlotCountMismatch {
                initial: 1,
                next: 2
            })
        );
    }

    #[test]
    fn truth_next_for_value_slot_is_rejected() {
        let mut iteration = doubling();
        iteration.next = vec![LocalId(3)];

        let kernel = iterating_kernel(iteration, Vec::new());

        assert_eq!(
            iteration_error(&kernel),
            Some(IterationError::SlotKindMismatch(0))
        );
    }

    #[test]
    fn next_outside_the_body_is_rejected() {
        let mut iteration = doubling();
        iteration.next = vec![LocalId(9)];

        let kernel = iterating_kernel(iteration, Vec::new());

        assert_eq!(
            iteration_error(&kernel),
            Some(IterationError::NextOutOfRange(LocalId(9)))
        );
    }

    #[test]
    fn exit_outside_the_body_is_rejected() {
        let mut iteration = doubling();
        iteration.exit = LocalId(9);

        let kernel = iterating_kernel(iteration, Vec::new());

        assert_eq!(
            iteration_error(&kernel),
            Some(IterationError::ExitOutOfRange(LocalId(9)))
        );
    }

    #[test]
    fn value_exit_is_rejected() {
        let mut iteration = doubling();
        iteration.exit = LocalId(2);

        let kernel = iterating_kernel(iteration, Vec::new());

        assert_eq!(
            iteration_error(&kernel),
            Some(IterationError::ExitNotTruth(LocalId(2)))
        );
    }
}
