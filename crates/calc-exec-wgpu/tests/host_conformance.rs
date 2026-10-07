#[path = "support/probe_cases.rs"]
mod probe_cases;

use std::time::Instant;

use calc_conformance::{
    Case, ConformanceError, ESCAPE_TIME_CASE_ITERATIONS, REDUCE_LENGTHS, check_bitwise,
    check_equivalent, check_expected, check_unrolled, constant_chain_case, contraction_case,
    escape_time_cases, escape_time_equivalent_cases, iteration_cases, operation_case, reduce_case,
};
use calc_exec::{
    Backend, Batch, Domain, EscapeTimePlanForm, OperationClass, PlanOp, PrepareError,
    ReduceOperation, ReduceShape, escape_time_iteration_plan,
};
use calc_exec_cpu::CpuBackend;
use calc_exec_gpu::{GpuBackend, OperationMode, probe};
use calc_exec_wgpu::{WgpuDevice, WgpuDeviceOptions};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

struct Tally {
    passed: usize,
    failed: usize,
    unsupported: usize,
}

impl Tally {
    fn record(&mut self, name: &str, result: Result<(), ConformanceError>) {
        match result {
            Ok(()) => {
                self.passed += 1;
                println!("pass {name}");
            }
            Err(ConformanceError::Prepare(PrepareError::Unsupported(reason))) => {
                self.unsupported += 1;
                println!("unsupported {name}: {reason:?}");
            }
            Err(error) => {
                self.failed += 1;
                println!("FAIL {name}: {error:?}");
            }
        }
    }
}

fn picture_case(iterations: u32) -> Case {
    let plan =
        escape_time_iteration_plan(Domain::F32, EscapeTimePlanForm::Parameter, iterations).unwrap();
    let mut real = Vec::new();
    let mut imaginary = Vec::new();
    for row in 0..HEIGHT {
        for column in 0..WIDTH {
            real.push(-2.5 + 3.5 * (f32::from(u16::try_from(column).unwrap()) + 0.5) / 1920.0);
            imaginary.push(-1.25 + 2.5 * (f32::from(u16::try_from(row).unwrap()) + 0.5) / 1080.0);
        }
    }
    let length = real.len();
    Case {
        plan,
        inputs: Batch::from_f32_columns(length, vec![real, imaginary]).unwrap(),
    }
}

fn timed_picture(backend: &dyn Backend, iterations: u32) -> (f64, Result<(), ConformanceError>) {
    let case = picture_case(iterations);
    let mut outputs = Batch::zeroed(Domain::F32, 2, case.inputs.len());
    let started = Instant::now();
    let run = backend
        .prepare(&case.plan)
        .map_err(ConformanceError::Prepare)
        .and_then(|mut prepared| {
            prepared
                .run(&case.inputs, &mut outputs)
                .map(|_| ())
                .map_err(ConformanceError::Run)
        });
    let seconds = started.elapsed().as_secs_f64();
    let checked = run.and_then(|()| check_bitwise(backend, &case));
    (seconds, checked)
}

fn main() {
    let device = match WgpuDevice::new(WgpuDeviceOptions::default()) {
        Ok(device) => device,
        Err(error) => {
            println!("no hardware device: {error:?}");
            std::process::exit(2);
        }
    };
    println!("adapter {:?}", device.description());
    let cases = probe_cases::cases();
    let started = Instant::now();
    let report = match probe(&device, &probe_cases::probe_cases(&cases)) {
        Ok(report) => report,
        Err(refusal) => {
            println!("probe refused the adapter: {refusal:?}");
            std::process::exit(1);
        }
    };
    println!("probe seconds {:.3}", started.elapsed().as_secs_f64());
    for outcome in &report.operations {
        let mode = match outcome.mode {
            OperationMode::Native => "native",
            OperationMode::IntegerExact => "integer-exact",
            OperationMode::ExactInBothForms => "exact-in-both-forms",
        };
        println!("mode {:?} {mode}", outcome.operation);
    }
    let backend = GpuBackend::with_modes(device, report.modes);
    let mut tally = Tally {
        passed: 0,
        failed: 0,
        unsupported: 0,
    };
    for operation in PlanOp::ALL {
        if operation.class() != OperationClass::CorrectlyRounded {
            continue;
        }
        if let Some(case) = operation_case(operation, Domain::F32) {
            tally.record(
                &format!("operation {operation:?}"),
                check_bitwise(&backend, &case),
            );
        }
    }
    if let Some(case) = contraction_case(Domain::F32) {
        tally.record("contraction", check_bitwise(&backend, &case));
    }
    if let Some(case) = constant_chain_case(Domain::F32) {
        tally.record("constant chain", check_bitwise(&backend, &case));
    }
    for operation in [
        ReduceOperation::Add,
        ReduceOperation::Mul,
        ReduceOperation::Min,
        ReduceOperation::Max,
    ] {
        for shape in [ReduceShape::LeftFold, ReduceShape::Halving] {
            for length in REDUCE_LENGTHS {
                if let Some(case) = reduce_case(operation, shape, length, Domain::F32) {
                    tally.record(
                        &format!("reduce {operation:?} {shape:?} {length}"),
                        check_bitwise(&backend, &case),
                    );
                }
            }
        }
    }
    for (index, case) in iteration_cases(Domain::F32).iter().enumerate() {
        tally.record(
            &format!("iteration case {index}"),
            check_unrolled(&backend, case),
        );
    }
    for iterations in [ESCAPE_TIME_CASE_ITERATIONS, 256] {
        for (index, case) in escape_time_equivalent_cases(Domain::F32, iterations)
            .iter()
            .enumerate()
        {
            tally.record(
                &format!("escape time form {index} at {iterations}"),
                check_equivalent(&backend, case),
            );
        }
    }
    for (index, case) in escape_time_cases(Domain::F32).iter().enumerate() {
        tally.record(
            &format!("escape time expected {index}"),
            check_expected(&backend, case),
        );
    }
    let cpu = CpuBackend::new();
    for iterations in [256_u32, 4096] {
        let (gpu_seconds, checked) = timed_picture(&backend, iterations);
        tally.record(&format!("picture 1920x1080 at {iterations}"), checked);
        let (cpu_seconds, _) = timed_picture(&cpu, iterations);
        println!(
            "timing 1920x1080 at {iterations}: gpu {gpu_seconds:.3} s, cpu one thread {cpu_seconds:.3} s"
        );
    }
    println!(
        "summary passed {} failed {} unsupported {}",
        tally.passed, tally.failed, tally.unsupported
    );
    if tally.failed > 0 {
        std::process::exit(1);
    }
}
