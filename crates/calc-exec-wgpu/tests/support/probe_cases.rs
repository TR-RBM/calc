use calc_conformance::{constant_chain_case, contraction_case, gpu_probe_case, operation_case};
use calc_exec::{Backend, Batch, Domain, Plan, PlanOp};
use calc_exec_cpu::CpuBackend;
use calc_exec_gpu::{ELIGIBLE_FOR_NATIVE, ProbeCase};

pub struct Case {
    pub operation: Option<PlanOp>,
    pub plan: Plan,
    pub inputs: Batch,
    pub expected: Batch,
}

fn with_reference(operation: Option<PlanOp>, plan: Plan, inputs: Batch) -> Option<Case> {
    let cpu = CpuBackend::new();
    let mut expected = Batch::zeroed(Domain::F32, plan.output_channel_count(), inputs.len());
    cpu.prepare(&plan).ok()?.run(&inputs, &mut expected).ok()?;
    Some(Case {
        operation,
        plan,
        inputs,
        expected,
    })
}

pub fn cases() -> Vec<Case> {
    let mut found = Vec::new();
    for operation in PlanOp::ALL {
        if let Some(case) = operation_case(operation, Domain::F32)
            && let Some(built) = with_reference(Some(operation), case.plan, case.inputs)
        {
            found.push(built);
        }
    }
    for case in [
        contraction_case(Domain::F32),
        constant_chain_case(Domain::F32),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(built) = with_reference(None, case.plan, case.inputs) {
            found.push(built);
        }
    }
    for operation in ELIGIBLE_FOR_NATIVE {
        if let Some(case) = gpu_probe_case(operation)
            && let Some(built) = with_reference(Some(operation), case.plan, case.inputs)
        {
            found.push(built);
        }
    }
    found
}

pub fn probe_cases(cases: &[Case]) -> Vec<ProbeCase<'_>> {
    cases
        .iter()
        .map(|case| ProbeCase {
            operation: case.operation,
            plan: &case.plan,
            inputs: &case.inputs,
            expected: &case.expected,
        })
        .collect()
}
