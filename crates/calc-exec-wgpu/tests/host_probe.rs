#[path = "support/probe_cases.rs"]
mod probe_cases;

use std::time::Instant;

use calc_exec_gpu::{OperationMode, probe};
use calc_exec_wgpu::{WgpuDevice, WgpuDeviceOptions};

use probe_cases::{cases, probe_cases};

fn main() {
    let device = match WgpuDevice::new(WgpuDeviceOptions::default()) {
        Ok(device) => device,
        Err(error) => {
            println!("no adapter: {error:?}");
            std::process::exit(2);
        }
    };
    println!("adapter {:?}", device.description());
    let cases = cases();
    let probe_cases = probe_cases(&cases);
    println!("cases {}", probe_cases.len());

    let started = Instant::now();
    let report = probe(&device, &probe_cases);
    let seconds = started.elapsed().as_secs_f64();

    match report {
        Ok(report) => {
            println!("probe seconds {seconds:.3}");
            println!("cases run {}", report.cases_run);
            for outcome in &report.operations {
                let mode = match outcome.mode {
                    OperationMode::Native => "native",
                    OperationMode::IntegerExact => "integer-exact",
                    OperationMode::ExactInBothForms => "exact-in-both-forms",
                };
                println!(
                    "mode {:?} {mode} cases {} subnormal {}",
                    outcome.operation, outcome.cases, outcome.subnormal_cases
                );
            }
            println!(
                "summary native {} integer-exact {} exact-in-both-forms {}",
                report
                    .operations
                    .iter()
                    .filter(|outcome| outcome.mode == OperationMode::Native)
                    .count(),
                report
                    .operations
                    .iter()
                    .filter(|outcome| outcome.mode == OperationMode::IntegerExact)
                    .count(),
                report
                    .operations
                    .iter()
                    .filter(|outcome| outcome.mode == OperationMode::ExactInBothForms)
                    .count()
            );
        }
        Err(refusal) => {
            println!("probe seconds {seconds:.3}");
            println!("refused {refusal:?}");
            std::process::exit(1);
        }
    }
}
