use calc_conformance::{constant_chain_case, contraction_case, gpu_probe_case, operation_case};
use std::sync::{Arc, Mutex};

use calc_exec::{
    Backend, BackendKind, Batch, Capabilities, Domain, ExecutionMode, Plan, PlanOp, PrepareError,
    Prepared, Unsupported,
};
use calc_exec_cpu::CpuBackend;
use calc_exec_gpu::{ELIGIBLE_FOR_NATIVE, GpuBackend, GpuDevice, ProbeCase, ProbeRefusal, probe};

use crate::result_record::{BackendUse, OperationModeUse};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuRefusal {
    NoAdapter,
    ExactCaseFailed(Option<PlanOp>),
    CaseNotRun(Option<PlanOp>),
}

pub struct GpuRegistration {
    backend: Option<Box<dyn Backend>>,
    modes: Vec<OperationModeUse>,
    refusal: Option<GpuRefusal>,
}

impl GpuRegistration {
    pub fn refused(refusal: GpuRefusal) -> GpuRegistration {
        GpuRegistration {
            backend: None,
            modes: Vec::new(),
            refusal: Some(refusal),
        }
    }

    pub fn refusal(&self) -> Option<&GpuRefusal> {
        self.refusal.as_ref()
    }

    pub fn modes(&self) -> &[OperationModeUse] {
        &self.modes
    }

    pub fn into_backend(self) -> Option<Box<dyn Backend>> {
        self.backend
    }
}

enum ProxyState {
    Unasked,
    Registered(Box<dyn Backend>),
    Refused(GpuRefusal),
}

struct SharedGpu(Arc<LazyGpuBackend>);

impl Backend for SharedGpu {
    fn kind(&self) -> BackendKind {
        self.0.kind()
    }

    fn capabilities(&self) -> &Capabilities {
        self.0.capabilities()
    }

    fn prepare(&self, plan: &Plan) -> Result<Box<dyn Prepared>, PrepareError> {
        self.0.prepare(plan)
    }

    fn execution_modes(&self) -> Vec<(PlanOp, ExecutionMode)> {
        self.0.execution_modes()
    }
}

pub struct LazyGpuBackend {
    capabilities: Capabilities,
    state: Mutex<ProxyState>,
    create: Box<dyn Fn() -> GpuRegistration + Send + Sync>,
}

impl LazyGpuBackend {
    pub fn new(create: Box<dyn Fn() -> GpuRegistration + Send + Sync>) -> LazyGpuBackend {
        LazyGpuBackend {
            capabilities: calc_exec_gpu::capabilities(),
            state: Mutex::new(ProxyState::Unasked),
            create,
        }
    }

    pub fn shared(
        create: Box<dyn Fn() -> GpuRegistration + Send + Sync>,
    ) -> (Box<dyn Backend>, Arc<LazyGpuBackend>) {
        let proxy = Arc::new(LazyGpuBackend::new(create));
        (Box::new(SharedGpu(Arc::clone(&proxy))), proxy)
    }

    pub fn refusal(&self) -> Option<GpuRefusal> {
        match self.state.lock() {
            Ok(state) => match &*state {
                ProxyState::Refused(refusal) => Some(refusal.clone()),
                ProxyState::Unasked | ProxyState::Registered(_) => None,
            },
            Err(_) => None,
        }
    }
}

impl Backend for LazyGpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn prepare(&self, plan: &Plan) -> Result<Box<dyn Prepared>, PrepareError> {
        let Ok(mut state) = self.state.lock() else {
            return Err(PrepareError::Unsupported(Unsupported::Domain(
                plan.domain(),
            )));
        };
        if let ProxyState::Unasked = &*state {
            let registration = (self.create)();
            let refusal = registration.refusal().cloned();
            *state = match (registration.into_backend(), refusal) {
                (Some(backend), _) => ProxyState::Registered(backend),
                (None, Some(refusal)) => ProxyState::Refused(refusal),
                (None, None) => ProxyState::Refused(GpuRefusal::NoAdapter),
            };
        }
        match &*state {
            ProxyState::Registered(backend) => backend.prepare(plan),
            ProxyState::Refused(_) => Err(PrepareError::Unsupported(Unsupported::DeviceRefused)),
            ProxyState::Unasked => Err(PrepareError::Unsupported(Unsupported::DeviceRefused)),
        }
    }

    fn execution_modes(&self) -> Vec<(PlanOp, ExecutionMode)> {
        match self.state.lock() {
            Ok(state) => match &*state {
                ProxyState::Registered(backend) => backend.execution_modes(),
                ProxyState::Unasked | ProxyState::Refused(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    }
}

struct ProbeCaseWithReference {
    operation: Option<PlanOp>,
    plan: Plan,
    inputs: Batch,
    expected: Batch,
}

fn with_reference(
    operation: Option<PlanOp>,
    plan: Plan,
    inputs: Batch,
) -> Option<ProbeCaseWithReference> {
    let cpu = CpuBackend::new();
    let mut expected = Batch::zeroed(Domain::F32, plan.output_channel_count(), inputs.len());
    cpu.prepare(&plan).ok()?.run(&inputs, &mut expected).ok()?;
    Some(ProbeCaseWithReference {
        operation,
        plan,
        inputs,
        expected,
    })
}

fn probe_cases() -> Vec<ProbeCaseWithReference> {
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

pub fn registration_of<D: GpuDevice>(device: D) -> GpuRegistration {
    let cases = probe_cases();
    let borrowed: Vec<ProbeCase<'_>> = cases
        .iter()
        .map(|case| ProbeCase {
            operation: case.operation,
            plan: &case.plan,
            inputs: &case.inputs,
            expected: &case.expected,
        })
        .collect();
    match probe(&device, &borrowed) {
        Ok(report) => {
            let backend = GpuBackend::with_modes(device, report.modes);
            let modes = BackendUse::modes_of(&backend);
            GpuRegistration {
                backend: Some(Box::new(backend)),
                modes,
                refusal: None,
            }
        }
        Err(ProbeRefusal::ExactCaseFailed(operation)) => {
            GpuRegistration::refused(GpuRefusal::ExactCaseFailed(operation))
        }
        Err(ProbeRefusal::CaseNotRun(operation)) => {
            GpuRegistration::refused(GpuRefusal::CaseNotRun(operation))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_exec::ExecutionMode;
    use calc_exec_gpu::{SoftwareDevice, SoftwareFault};

    fn mode_of(registration: &GpuRegistration, operation: PlanOp) -> Option<ExecutionMode> {
        registration
            .modes()
            .iter()
            .find(|entry| entry.operation == operation)
            .map(|entry| entry.mode)
    }

    #[test]
    fn a_proxy_asks_for_no_device_until_a_plan_selects_it() {
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = Arc::clone(&asked);
        let (backend, _proxy) = LazyGpuBackend::shared(Box::new(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            GpuRegistration::refused(GpuRefusal::NoAdapter)
        }));

        let _ = backend.capabilities().domains.len();
        let _ = backend.kind();
        let modes = backend.execution_modes();

        assert_eq!(
            (asked.load(std::sync::atomic::Ordering::SeqCst), modes.len()),
            (0, 0)
        );
    }

    #[test]
    fn a_proxy_asks_once_and_remembers_a_refusal() {
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = Arc::clone(&asked);
        let (backend, proxy) = LazyGpuBackend::shared(Box::new(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            GpuRegistration::refused(GpuRefusal::NoAdapter)
        }));
        let plan = calc_conformance::operation_case(PlanOp::Add, Domain::F32)
            .expect("an add case")
            .plan;

        let first = backend.prepare(&plan);
        let second = backend.prepare(&plan);

        assert!(matches!(
            (first, second),
            (
                Err(PrepareError::Unsupported(Unsupported::DeviceRefused)),
                Err(PrepareError::Unsupported(Unsupported::DeviceRefused))
            )
        ));
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(proxy.refusal(), Some(GpuRefusal::NoAdapter));
    }

    #[test]
    fn a_proxy_with_a_device_reports_the_device_modes() {
        let (backend, _proxy) =
            LazyGpuBackend::shared(Box::new(|| registration_of(SoftwareDevice::new(1))));
        let plan = calc_conformance::operation_case(PlanOp::Add, Domain::F32)
            .expect("an add case")
            .plan;

        let before = backend.execution_modes().len();
        let prepared = backend.prepare(&plan).is_ok();
        let after = backend.execution_modes().len();

        assert!(prepared && before == 0 && after > 0);
    }

    #[test]
    fn a_device_that_passes_every_case_is_registered() {
        let registration = registration_of(SoftwareDevice::new(1));

        assert!(registration.refusal().is_none() && registration.into_backend().is_some());
    }

    #[test]
    fn registration_carries_the_mode_the_probe_chose() {
        let registration = registration_of(SoftwareDevice::new(1));

        assert_eq!(
            mode_of(&registration, PlanOp::Floor),
            Some(ExecutionMode::Native)
        );
    }

    #[test]
    fn a_device_that_flushes_subnormals_natively_is_registered_in_the_exact_mode() {
        let device = SoftwareDevice::with_fault(1, SoftwareFault::FlushesSubnormalsNatively);

        let registration = registration_of(device);

        assert_eq!(
            mode_of(&registration, PlanOp::Ceil),
            Some(ExecutionMode::IntegerExact)
        );
    }

    #[test]
    fn a_device_that_fails_an_exact_case_is_not_registered() {
        let device = SoftwareDevice::with_fault(1, SoftwareFault::WrongResultAtDispatch);

        let registration = registration_of(device);

        assert!(registration.refusal().is_some() && registration.into_backend().is_none());
    }

    #[test]
    fn a_bit_operation_is_exact_in_both_forms() {
        let registration = registration_of(SoftwareDevice::new(1));

        assert_eq!(
            mode_of(&registration, PlanOp::CopySign),
            Some(ExecutionMode::ExactInBothForms)
        );
    }
}
