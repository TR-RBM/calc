# calc-conformance

## What it does

Shared test cases that run one plan on a backend and compare the result with `calc-exec-cpu`. Used only as a development dependency of backend crates.

- `operation_case(operation, domain)` builds a plan for one `PlanOp`. The input batch is the cross product of the special values for every argument: both zeros, both infinities, NaN of both signs, the smallest and largest subnormal, the smallest normal value, `MAX`, `MIN`, 1 and its two neighbours, and rounding ties. Boolean arguments come from comparing an input with 1. Boolean results are turned into 1 or 0 by `Select`.
- `reduce_case(operation, shape, length, domain)` builds a reduce plan over values on which the two tree shapes give different results. `REDUCE_LENGTHS` lists the lengths the execution layer requires.
- `contraction_case(domain)` builds `Add(Mul(a, b), c)` on values where a fused multiply-add gives a different result.
- `check_bitwise(backend, case)` runs the case on the reference and on the backend and returns the first output whose bits differ. Any NaN matches any NaN. Signed zeros must match.
- `check_bound(backend, case)` is the check for approximate operations. The reference implementations are correctly rounded, so their documented bound is half an ulp, and a result that is correctly rounded with ties to even equals the reference result. `check_bound` therefore reports the first output whose bits differ from the reference as `OutsideBound`. It never uses a tolerance.
- `corner_cases(domain)` returns cases for `Atan2` and `Pow` with independent expected results, so a fault in the reference itself is also caught: `atan2` of `MAX` over the smallest subnormal and of the smallest subnormal over `-MAX` in all sign combinations, which must give ±π/2, ±π or 0, and `pow` of -1 to `±MAX` and to 3, which must give 1 and -1. An independent oracle found them wrong in `atan2_f32` and `pow_f32`.
- `check_expected(backend, case)` compares a backend with the expected results of such a case: the same bits for `F64`, and for `F32`, whose references are bounded by one ulp, the expected value or one of its two neighbouring machine numbers.

- `iteration_cases(domain)` returns plans with an iteration gate: a doubling and a squaring scalar slot, the doubling with `maximum_count` 0, a boolean slot beside a scalar one, and a sine step, on inputs whose exit holds at step 0, part way and never, including NaN, both infinities, `MAX` and a subnormal value. `check_unrolled(backend, case)` runs the case on the backend and `Plan::unrolled` of it on the reference. It compares bits for every plan, including a body with an approximate operation, because a backend runs such a plan only with the reference implementations.
- `escape_time_equivalent_cases(domain, iterations)` pairs the iteration form of an escape-time picture with the unrolled escape-time circuit, and `check_equivalent(backend, case)` compares the backend on the first with the reference on the second.

- `check_worker_counts(build, case, seed)` builds the backend once with `SharedParallelism::sequential()` as the reference, and once for each of `WORKER_COUNTS` (1, 2, 3 and 8) with `PermutedParallelism`, which runs the tasks on one thread in an order permuted by the seed. It compares every build bit for bit with the reference, so a result that depends on the worker count or on task order shows up as a reproducible `Mismatch`.

A backend crate calls these in its own unit tests, one case per test.

## How to test

`cargo test -p calc-conformance`

The tests show that the checks notice faults. A backend that evaluates `LeftFold` as `Halving`, one that contracts `Mul` then `Add` into a fused multiply-add, and one that returns `+0` for `-0` each fail `check_bitwise`. A backend that flips the sign of NaN passes. A backend one ulp above the reference fails `check_bound`. A backend that negates results fails `check_expected` on the `atan2` corners, the reference passes its own operation cases in both domains and the `F64` corner cases, and the `F32` expected results equal the `F32` references of `calc-numbers`. A backend that ignores the exit of an iteration, and one that takes one step more than its maximum count, fail `check_unrolled`, a backend one ulp above the reference fails `check_unrolled` on the sine case with `Mismatch`, and a backend that negates results fails `check_equivalent`. The CPU backend with chunks of 7 elements passes `check_worker_counts` on every operation case in both domains, on reduce cases of length 1025 and on every iteration and escape-time case, for two seeds, and a backend whose results change above two workers fails it.
