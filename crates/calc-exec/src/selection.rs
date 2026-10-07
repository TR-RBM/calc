use crate::backend::{Backend, BackendKind, PrepareError, Prepared, Unsupported};
use crate::plan::Plan;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    Automatic,
    Only(BackendKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    Unsupported(Unsupported),
    BatchTooLong { requested: usize, largest: usize },
    PrepareFailed(PrepareError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkippedBackend {
    pub backend_index: usize,
    pub kind: BackendKind,
    pub reason: SkipReason,
}

pub struct Selection {
    pub backend_index: usize,
    pub kind: BackendKind,
    pub prepared: Box<dyn Prepared>,
    pub skipped: Vec<SkippedBackend>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectError {
    NotRegistered(BackendKind),
    Rejected {
        kind: BackendKind,
        reason: SkipReason,
    },
    NoBackend {
        skipped: Vec<SkippedBackend>,
    },
}

fn capability_problem(
    backend: &dyn Backend,
    plan: &Plan,
    batch_length: usize,
) -> Option<SkipReason> {
    let capabilities = backend.capabilities();
    if let Err(unsupported) = capabilities.check_plan(plan) {
        return Some(SkipReason::Unsupported(unsupported));
    }
    if batch_length > capabilities.largest_batch_length {
        return Some(SkipReason::BatchTooLong {
            requested: batch_length,
            largest: capabilities.largest_batch_length,
        });
    }
    None
}

pub fn select(
    backends: &[&dyn Backend],
    plan: &Plan,
    batch_length: usize,
    preference: Preference,
) -> Result<Selection, SelectError> {
    match preference {
        Preference::Only(kind) => select_only(backends, plan, batch_length, kind),
        Preference::Automatic => select_automatic(backends, plan, batch_length),
    }
}

fn select_only(
    backends: &[&dyn Backend],
    plan: &Plan,
    batch_length: usize,
    kind: BackendKind,
) -> Result<Selection, SelectError> {
    let Some((backend_index, backend)) = backends
        .iter()
        .enumerate()
        .find(|(_, backend)| backend.kind() == kind)
    else {
        return Err(SelectError::NotRegistered(kind));
    };
    if let Some(reason) = capability_problem(*backend, plan, batch_length) {
        return Err(SelectError::Rejected { kind, reason });
    }
    match backend.prepare(plan) {
        Ok(prepared) => Ok(Selection {
            backend_index,
            kind,
            prepared,
            skipped: Vec::new(),
        }),
        Err(error) => Err(SelectError::Rejected {
            kind,
            reason: SkipReason::PrepareFailed(error),
        }),
    }
}

fn select_automatic(
    backends: &[&dyn Backend],
    plan: &Plan,
    batch_length: usize,
) -> Result<Selection, SelectError> {
    let mut skipped = Vec::new();
    let mut candidates = Vec::new();
    for (backend_index, backend) in backends.iter().enumerate() {
        match capability_problem(*backend, plan, batch_length) {
            Some(reason) => skipped.push(SkippedBackend {
                backend_index,
                kind: backend.kind(),
                reason,
            }),
            None => candidates.push((
                backend
                    .capabilities()
                    .estimate_picoseconds(plan, batch_length),
                backend_index,
            )),
        }
    }
    candidates.sort_by_key(|(estimate, _)| *estimate);
    for (_, backend_index) in candidates {
        let Some(backend) = backends.get(backend_index) else {
            continue;
        };
        match backend.prepare(plan) {
            Ok(prepared) => {
                return Ok(Selection {
                    backend_index,
                    kind: backend.kind(),
                    prepared,
                    skipped,
                });
            }
            Err(error @ PrepareError::Unsupported(_)) => skipped.push(SkippedBackend {
                backend_index,
                kind: backend.kind(),
                reason: SkipReason::PrepareFailed(error),
            }),
            Err(
                error @ (PrepareError::OutOfDeviceMemory
                | PrepareError::DeviceLost
                | PrepareError::KernelRejected),
            ) => {
                return Err(SelectError::Rejected {
                    kind: backend.kind(),
                    reason: SkipReason::PrepareFailed(error),
                });
            }
        }
    }
    Err(SelectError::NoBackend { skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Capabilities, CostParameters, RunError, RunReport, TransferCost};
    use crate::batch::Batch;
    use crate::domain::Domain;
    use crate::operation::PlanOp;
    use crate::plan::{ChannelId, Gate, GateId};

    struct IdlePrepared;

    impl Prepared for IdlePrepared {
        fn run(&mut self, _inputs: &Batch, _outputs: &mut Batch) -> Result<RunReport, RunError> {
            Ok(RunReport::default())
        }
    }

    struct TestBackend {
        kind: BackendKind,
        capabilities: Capabilities,
        refuses_prepare: bool,
        prepare_failure: Option<PrepareError>,
    }

    impl Backend for TestBackend {
        fn kind(&self) -> BackendKind {
            self.kind
        }

        fn capabilities(&self) -> &Capabilities {
            &self.capabilities
        }

        fn prepare(&self, _plan: &Plan) -> Result<Box<dyn Prepared>, PrepareError> {
            if let Some(failure) = self.prepare_failure {
                return Err(failure);
            }
            if self.refuses_prepare {
                Err(PrepareError::Unsupported(Unsupported::Operation(
                    PlanOp::Add,
                )))
            } else {
                Ok(Box::new(IdlePrepared))
            }
        }
    }

    fn backend(kind: BackendKind, setup: u64, per_gate: u64) -> TestBackend {
        TestBackend {
            kind,
            capabilities: Capabilities {
                domains: vec![Domain::F32, Domain::F64],
                operations: PlanOp::ALL.into_iter().collect(),
                largest_batch_length: 1 << 20,
                largest_iteration_count: u32::MAX,
                reference_approximate_operations: false,
                cost: CostParameters {
                    setup_picoseconds: setup,
                    picoseconds_per_gate_evaluation: per_gate,
                    transfer: None,
                },
            },
            refuses_prepare: false,
            prepare_failure: None,
        }
    }

    #[test]
    fn the_same_question_chooses_the_same_backend_every_time() {
        let cpu = backend(
            BackendKind::Cpu,
            crate::cost::CPU.setup_picoseconds,
            crate::cost::CPU.picoseconds_per_gate_evaluation,
        );
        let simd = backend(
            BackendKind::Simd,
            crate::cost::SIMD.setup_picoseconds,
            crate::cost::SIMD.picoseconds_per_gate_evaluation,
        );
        let backends: Vec<&dyn Backend> = vec![&cpu, &simd];
        let plan = add_plan();

        for length in [1usize, 2, 16, 1_024, 1 << 20] {
            let first = select(&backends, &plan, length, Preference::Automatic)
                .map(|selection| (selection.backend_index, selection.kind));
            for _ in 0..16 {
                let again = select(&backends, &plan, length, Preference::Automatic)
                    .map(|selection| (selection.backend_index, selection.kind));
                assert_eq!(again, first, "length {length}");
            }
        }
    }

    #[test]
    fn the_measured_constants_keep_one_value_on_the_cpu_and_send_a_long_batch_to_simd() {
        let cpu = backend(
            BackendKind::Cpu,
            crate::cost::CPU.setup_picoseconds,
            crate::cost::CPU.picoseconds_per_gate_evaluation,
        );
        let simd = backend(
            BackendKind::Simd,
            crate::cost::SIMD.setup_picoseconds,
            crate::cost::SIMD.picoseconds_per_gate_evaluation,
        );
        let backends: Vec<&dyn Backend> = vec![&cpu, &simd];
        let plan = add_plan();

        let one = select(&backends, &plan, 1, Preference::Automatic);
        let many = select(&backends, &plan, 1 << 20, Preference::Automatic);

        assert_eq!(
            (
                one.map(|selection| selection.kind),
                many.map(|selection| selection.kind)
            ),
            (Ok(BackendKind::Cpu), Ok(BackendKind::Simd))
        );
    }

    #[test]
    fn a_picture_goes_to_the_gpu_and_a_small_batch_stays_on_the_cpu() {
        let cpu = backend(
            BackendKind::Cpu,
            crate::cost::CPU.setup_picoseconds,
            crate::cost::CPU.picoseconds_per_gate_evaluation,
        );
        let simd = backend(
            BackendKind::Simd,
            crate::cost::SIMD.setup_picoseconds,
            crate::cost::SIMD.picoseconds_per_gate_evaluation,
        );
        let mut gpu = backend(
            BackendKind::Gpu,
            crate::cost::GPU.setup_picoseconds,
            crate::cost::GPU.picoseconds_per_gate_evaluation,
        );
        gpu.capabilities.cost.transfer = crate::cost::GPU.transfer;
        gpu.capabilities.largest_batch_length = 1 << 21;
        let backends: Vec<&dyn Backend> = vec![&cpu, &simd, &gpu];
        let plan = crate::escape_time::escape_time_iteration_plan(
            Domain::F32,
            crate::escape_time::EscapeTimePlanForm::Parameter,
            4_096,
        )
        .expect("an escape time plan");

        let chosen = [1usize, 1_000, 1920 * 1080].map(|length| {
            select(&backends, &plan, length, Preference::Automatic)
                .map(|selection| selection.kind)
                .expect("a backend")
        });

        assert_eq!(
            chosen,
            [BackendKind::Cpu, BackendKind::Simd, BackendKind::Gpu]
        );
    }

    fn add_plan() -> Plan {
        Plan::new(
            Domain::F64,
            2,
            vec![
                Gate::Input(ChannelId(0)),
                Gate::Input(ChannelId(1)),
                Gate::Operation {
                    operation: PlanOp::Add,
                    arguments: vec![GateId(0), GateId(1)],
                },
            ],
            vec![GateId(2)],
            None,
        )
        .unwrap()
    }

    fn chosen(result: Result<Selection, SelectError>) -> (usize, Vec<SkippedBackend>) {
        match result {
            Ok(selection) => (selection.backend_index, selection.skipped),
            Err(error) => panic!("selection failed: {error:?}"),
        }
    }

    fn failure(result: Result<Selection, SelectError>) -> SelectError {
        match result {
            Ok(selection) => panic!("selected backend {}", selection.backend_index),
            Err(error) => error,
        }
    }

    #[test]
    fn automatic_chooses_smallest_estimate() {
        let cpu = backend(BackendKind::Cpu, 0, 10);
        let simd = backend(BackendKind::Simd, 0, 2);

        let result = select(&[&cpu, &simd], &add_plan(), 100, Preference::Automatic);

        assert_eq!(chosen(result).0, 1);
    }

    #[test]
    fn automatic_tie_goes_to_first_registered() {
        let cpu = backend(BackendKind::Cpu, 5, 1);
        let simd = backend(BackendKind::Simd, 5, 1);

        let result = select(&[&cpu, &simd], &add_plan(), 100, Preference::Automatic);

        assert_eq!(chosen(result).0, 0);
    }

    #[test]
    fn automatic_keeps_small_batch_on_host_when_transfer_dominates() {
        let cpu = backend(BackendKind::Cpu, 0, 10);
        let mut gpu = backend(BackendKind::Gpu, 1_000, 0);
        gpu.capabilities.cost.transfer = Some(TransferCost {
            latency_picoseconds: 50_000,
            picoseconds_per_entry: 1_000_000_000,
        });

        let result = select(&[&cpu, &gpu], &add_plan(), 4, Preference::Automatic);

        assert_eq!(chosen(result).0, 0);
    }

    #[test]
    fn automatic_skips_backend_without_domain() {
        let mut gpu = backend(BackendKind::Gpu, 0, 0);
        gpu.capabilities.domains = vec![Domain::F32];
        let cpu = backend(BackendKind::Cpu, 0, 10);

        let result = select(&[&gpu, &cpu], &add_plan(), 100, Preference::Automatic);

        assert_eq!(
            chosen(result),
            (
                1,
                vec![SkippedBackend {
                    backend_index: 0,
                    kind: BackendKind::Gpu,
                    reason: SkipReason::Unsupported(Unsupported::Domain(Domain::F64))
                }]
            )
        );
    }

    #[test]
    fn automatic_skips_backend_without_operation() {
        let mut simd = backend(BackendKind::Simd, 0, 0);
        simd.capabilities.operations = vec![PlanOp::Mul];
        let cpu = backend(BackendKind::Cpu, 0, 10);

        let result = select(&[&cpu, &simd], &add_plan(), 100, Preference::Automatic);

        assert_eq!(
            chosen(result).1,
            vec![SkippedBackend {
                backend_index: 1,
                kind: BackendKind::Simd,
                reason: SkipReason::Unsupported(Unsupported::Operation(PlanOp::Add))
            }]
        );
    }

    #[test]
    fn automatic_skips_backend_with_smaller_batch_limit() {
        let cpu = backend(BackendKind::Cpu, 0, 10);
        let mut gpu = backend(BackendKind::Gpu, 0, 0);
        gpu.capabilities.largest_batch_length = 50;

        let result = select(&[&cpu, &gpu], &add_plan(), 100, Preference::Automatic);

        assert_eq!(
            chosen(result).1,
            vec![SkippedBackend {
                backend_index: 1,
                kind: BackendKind::Gpu,
                reason: SkipReason::BatchTooLong {
                    requested: 100,
                    largest: 50
                }
            }]
        );
    }

    #[test]
    fn automatic_tries_next_estimate_when_prepare_is_unsupported() {
        let cpu = backend(BackendKind::Cpu, 0, 10);
        let mut simd = backend(BackendKind::Simd, 0, 1);
        simd.refuses_prepare = true;

        let result = select(&[&cpu, &simd], &add_plan(), 100, Preference::Automatic);

        assert_eq!(
            chosen(result),
            (
                0,
                vec![SkippedBackend {
                    backend_index: 1,
                    kind: BackendKind::Simd,
                    reason: SkipReason::PrepareFailed(PrepareError::Unsupported(
                        Unsupported::Operation(PlanOp::Add)
                    ))
                }]
            )
        );
    }

    #[test]
    fn automatic_stops_when_prepare_loses_the_device() {
        let cpu = backend(BackendKind::Cpu, 0, 10);
        let mut gpu = backend(BackendKind::Gpu, 0, 1);
        gpu.prepare_failure = Some(PrepareError::DeviceLost);

        let result = select(&[&cpu, &gpu], &add_plan(), 100, Preference::Automatic);

        assert_eq!(
            failure(result),
            SelectError::Rejected {
                kind: BackendKind::Gpu,
                reason: SkipReason::PrepareFailed(PrepareError::DeviceLost)
            }
        );
    }

    #[test]
    fn automatic_without_capable_backend_fails_with_all_skips() {
        let mut cpu = backend(BackendKind::Cpu, 0, 1);
        cpu.capabilities.domains = vec![Domain::F32];

        let result = select(&[&cpu], &add_plan(), 100, Preference::Automatic);

        assert_eq!(
            failure(result),
            SelectError::NoBackend {
                skipped: vec![SkippedBackend {
                    backend_index: 0,
                    kind: BackendKind::Cpu,
                    reason: SkipReason::Unsupported(Unsupported::Domain(Domain::F64))
                }]
            }
        );
    }

    #[test]
    fn automatic_selection_is_repeatable() {
        let cpu = backend(BackendKind::Cpu, 7, 3);
        let simd = backend(BackendKind::Simd, 1, 4);
        let gpu = backend(BackendKind::Gpu, 3, 3);
        let backends: [&dyn Backend; 3] = [&cpu, &simd, &gpu];

        let first = chosen(select(&backends, &add_plan(), 6, Preference::Automatic));
        let second = chosen(select(&backends, &add_plan(), 6, Preference::Automatic));

        assert_eq!(first, second);
    }

    #[test]
    fn only_uses_named_backend_even_when_slower() {
        let cpu = backend(BackendKind::Cpu, 0, 1);
        let gpu = backend(BackendKind::Gpu, 1_000_000, 1);

        let result = select(
            &[&cpu, &gpu],
            &add_plan(),
            10,
            Preference::Only(BackendKind::Gpu),
        );

        assert_eq!(chosen(result).0, 1);
    }

    #[test]
    fn only_with_unregistered_kind_fails() {
        let cpu = backend(BackendKind::Cpu, 0, 1);

        let result = select(
            &[&cpu],
            &add_plan(),
            10,
            Preference::Only(BackendKind::Simd),
        );

        assert_eq!(
            failure(result),
            SelectError::NotRegistered(BackendKind::Simd)
        );
    }

    #[test]
    fn only_with_uncovered_plan_fails_without_fallback() {
        let cpu = backend(BackendKind::Cpu, 0, 1);
        let mut gpu = backend(BackendKind::Gpu, 0, 1);
        gpu.capabilities.domains = vec![Domain::F32];

        let result = select(
            &[&cpu, &gpu],
            &add_plan(),
            10,
            Preference::Only(BackendKind::Gpu),
        );

        assert_eq!(
            failure(result),
            SelectError::Rejected {
                kind: BackendKind::Gpu,
                reason: SkipReason::Unsupported(Unsupported::Domain(Domain::F64))
            }
        );
    }

    #[test]
    fn only_with_refused_prepare_fails_without_fallback() {
        let cpu = backend(BackendKind::Cpu, 0, 1);
        let mut simd = backend(BackendKind::Simd, 0, 1);
        simd.refuses_prepare = true;

        let result = select(
            &[&cpu, &simd],
            &add_plan(),
            10,
            Preference::Only(BackendKind::Simd),
        );

        assert_eq!(
            failure(result),
            SelectError::Rejected {
                kind: BackendKind::Simd,
                reason: SkipReason::PrepareFailed(PrepareError::Unsupported(
                    Unsupported::Operation(PlanOp::Add)
                ))
            }
        );
    }
}
