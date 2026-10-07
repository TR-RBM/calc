use calc_numbers::exp_f64;
use std::hint::black_box;
use std::sync::Barrier;
use std::time::Instant;

#[test]
fn concurrent_first_use_keeps_reference_bits() {
    let barrier = Barrier::new(8);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                for _ in 0..64 {
                    assert_eq!(
                        exp_f64(-0.010195950715390869).to_bits(),
                        0x3fef_ace6_2de5_0f3b
                    );
                    assert_eq!(
                        exp_f64(-1.6322261495516757).to_bits(),
                        0x3fc9_05f1_9d9b_b2ea
                    );
                    assert_eq!(exp_f64(1.0).to_bits(), std::f64::consts::E.to_bits());
                }
            });
        }
    });
}

#[test]
#[ignore]
fn profile_concurrent_first_use() {
    for (argument, expected) in [
        (-0.010195950715390869, 0x3fef_ace6_2de5_0f3b),
        (-1.6322261495516757, 0x3fc9_05f1_9d9b_b2ea),
    ] {
        let barrier = Barrier::new(8);
        let started = Instant::now();
        let durations = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        let thread_started = Instant::now();
                        assert_eq!(exp_f64(argument).to_bits(), expected);
                        let first = thread_started.elapsed();
                        let thread_started = Instant::now();
                        for _ in 0..64 {
                            assert_eq!(exp_f64(argument).to_bits(), expected);
                        }
                        (first.as_nanos(), thread_started.elapsed().as_nanos() / 64)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        let whole = started.elapsed();
        eprintln!(
            "argument_bits={:016x} threads=8 first_min_ns={} first_max_ns={} warm_min_mean_ns={} warm_max_mean_ns={} whole_ns={}",
            argument.to_bits(),
            durations.iter().map(|value| value.0).min().unwrap(),
            durations.iter().map(|value| value.0).max().unwrap(),
            durations.iter().map(|value| value.1).min().unwrap(),
            durations.iter().map(|value| value.1).max().unwrap(),
            whole.as_nanos(),
        );
    }
}

#[test]
#[ignore]
fn reconstructed_solver_exponential_calls_match_pinned_bits() {
    let source = include_str!("fixtures/ln2-solver-calls.csv");
    let calls: Vec<_> = source
        .lines()
        .skip(1)
        .map(|line| {
            let cells: Vec<_> = line.split(',').collect();
            let argument = f64::from_bits(u64::from_str_radix(cells[2], 16).unwrap());
            let result = u64::from_str_radix(cells[3], 16).unwrap();
            (argument, result)
        })
        .collect();
    let started = Instant::now();
    for (argument, expected) in &calls {
        assert_eq!(
            black_box(exp_f64(black_box(*argument))).to_bits(),
            *expected
        );
    }
    let cold = started.elapsed();
    let repeats = 8;
    let started = Instant::now();
    for _ in 0..repeats {
        for (argument, expected) in &calls {
            assert_eq!(
                black_box(exp_f64(black_box(*argument))).to_bits(),
                *expected
            );
        }
    }
    let warm = started.elapsed();
    eprintln!(
        "calls={} cold_total_ns={} warm_mean_call_ns={} repeats={repeats}",
        calls.len(),
        cold.as_nanos(),
        warm.as_nanos() / (calls.len() * repeats) as u128
    );
}
