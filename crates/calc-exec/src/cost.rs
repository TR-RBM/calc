use crate::backend::{CostParameters, TransferCost};

pub const CPU: CostParameters = CostParameters {
    setup_picoseconds: 741_000,
    picoseconds_per_gate_evaluation: 3_813,
    transfer: None,
};

pub const SIMD: CostParameters = CostParameters {
    setup_picoseconds: 1_683_000,
    picoseconds_per_gate_evaluation: 1_363,
    transfer: None,
};

pub const GPU: CostParameters = CostParameters {
    setup_picoseconds: 118_774_000,
    picoseconds_per_gate_evaluation: 36,
    transfer: Some(TransferCost {
        latency_picoseconds: 33_971_500,
        picoseconds_per_entry: 4_119,
    }),
};

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL_RUNS: [&str; 3] = [
        include_str!("../tests/fixtures/cost-runs/local-1.txt"),
        include_str!("../tests/fixtures/cost-runs/local-2.txt"),
        include_str!("../tests/fixtures/cost-runs/local-3.txt"),
    ];

    const HOST_RUNS: [&str; 3] = [
        include_str!("../tests/fixtures/cost-runs/host-1.txt"),
        include_str!("../tests/fixtures/cost-runs/host-2.txt"),
        include_str!("../tests/fixtures/cost-runs/host-3.txt"),
    ];

    fn smallest(runs: &[&str], backend: &str, parameter: &str) -> u64 {
        let mut found = u64::MAX;
        for run in runs {
            for line in run.lines() {
                let mut words = line.split_whitespace();
                if words.next() != Some("fit") || words.next() != Some(backend) {
                    continue;
                }
                let mut rest = words;
                while let Some(word) = rest.next() {
                    if word == parameter
                        && let Some(value) = rest.next().and_then(|text| text.parse::<u64>().ok())
                    {
                        found = found.min(value);
                    }
                }
            }
        }
        found
    }

    #[test]
    fn every_constant_is_the_smallest_measurement_in_the_evidence() {
        let measured = [
            ("cpu", "setup_picoseconds", CPU.setup_picoseconds, false),
            (
                "cpu",
                "picoseconds_per_gate_evaluation",
                CPU.picoseconds_per_gate_evaluation,
                false,
            ),
            ("simd", "setup_picoseconds", SIMD.setup_picoseconds, false),
            (
                "simd",
                "picoseconds_per_gate_evaluation",
                SIMD.picoseconds_per_gate_evaluation,
                false,
            ),
            ("gpu", "setup_picoseconds", GPU.setup_picoseconds, true),
            (
                "gpu",
                "picoseconds_per_gate_evaluation",
                GPU.picoseconds_per_gate_evaluation,
                true,
            ),
        ];

        for (backend, parameter, committed, on_host) in measured {
            let runs: &[&str] = if on_host { &HOST_RUNS } else { &LOCAL_RUNS };
            assert_eq!(
                committed,
                smallest(runs, backend, parameter),
                "{backend} {parameter}"
            );
        }
    }

    #[test]
    fn the_transfer_constants_are_the_smallest_measurement_in_the_evidence() {
        let transfer = GPU.transfer.expect("the GPU moves its own memory");

        assert_eq!(
            (transfer.latency_picoseconds, transfer.picoseconds_per_entry),
            (
                smallest(&HOST_RUNS, "gpu", "latency_picoseconds"),
                smallest(&HOST_RUNS, "gpu", "picoseconds_per_entry")
            )
        );
    }

    #[test]
    fn only_the_gpu_pays_for_transfer() {
        let found = [CPU.transfer, SIMD.transfer, GPU.transfer].map(|cost| cost.is_some());

        assert_eq!(found, [false, false, true]);
    }
}
