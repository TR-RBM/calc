# calc-exec-simd

## What it does

The SIMD backend of the execution layer, for x86_64. It compiles a plan once into a program of steps, then evaluates that program over lanes: four `f32` or two `f64` on the SSE2 baseline, eight or four under AVX2, chosen by run-time detection in `InstructionSet::detected`.

Every result is the same bits as `calc-exec-cpu` produces. The arithmetic instructions are correctly rounded, so they match on their own; `minimum`, `maximum` and the comparisons are built from compares and blends because the hardware's own minimum and maximum disagree with IEEE on NaN and on the sign of zero; rounding to an integer, the fused multiply-add and the approximate operations run per lane through the same `calc-numbers` functions the CPU backend calls, so they cannot drift from it. Reductions follow the shape of the plan rather than the width of a register: `LeftFold` stays sequential and `Halving` builds the recursive split.

The intrinsics live in one module per instruction set, `sse2` and `avx2`, each declared `#[allow(unsafe_code)]`. Nothing else in the crate lowers the lint.

## How to test

`cargo test -p calc-exec-simd` runs every case of `calc-conformance` against the CPU backend for bit equality, in both domains and on every instruction set this machine has: the operation cases, the corner cases, contraction, the constant chain, the reductions at every shape and length, the iteration cases and the escape-time cases.

The cost model in `SETUP_NANOSECONDS` and `NANOSECONDS_PER_GATE_EVALUATION` comes from measuring it against the CPU backend.
