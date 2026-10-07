# calc-exec-cpu

## What it does

The scalar reference backend. Its results define the expected results of every other backend.

`CpuBackend` implements `Backend` for `F32` and `F64` plans. `prepare` compiles a plan into a list of steps, one per gate. `run` checks the batch shapes, evaluates the map stage gate by gate in gate order for each element, and evaluates a reduce stage by building exactly the named tree. `LeftFold` combines from the left. `Halving` splits a run of n values into the first floor(n/2) and the remaining ceil(n/2) values.

Arithmetic, `Sqrt`, `MulAdd`, rounding and `CopySign` use the correctly rounded standard library operations. `Min` and `Max` use the IEEE `minimum` and `maximum` of `calc-numbers`. `CpuBackend::new` runs on the calling thread. `CpuBackend::with_parallelism` takes the `SharedParallelism` at registration and splits the map stage into contiguous chunks, one task per chunk through `run_all`. A chunk holds at least enough elements for 65 536 gate evaluations, so cheap plans stay in one chunk and costly plans such as escape-time split even a batch of 4096 positions. Each chunk writes its own output columns, which are joined in element order after `run_all` returns, and the reduce stage runs on the joined column in its tree shape. The output bits therefore do not depend on the worker count or on the order in which tasks run. A parallelism that returns without running every task gives `RunError::ParallelTaskNotRun`. `with_chunk_length` fixes the minimum chunk length, for tests. An iteration gate runs per element: the state starts from its initial gates, the body is evaluated, and the element stops at the first state whose exit holds or after the maximum count, so an element that exits early costs only its own steps.

Approximate operations in `F64` plans call the correctly rounded reference implementations of `calc-numbers`: `exp_f64`, `ln_f64`, `sin_f64`, `cos_f64`, `tan_f64`, `atan2_f64` and `pow_f64`. In `F32` plans they call the references of `calc-numbers` that compute in `f32` arithmetic: `exp_f32`, `ln_f32`, `sin_f32`, `cos_f32`, `tan_f32`, `atan2_f32` and `pow_f32`, each within one ulp of the exact value. A reduced plan run on an empty batch returns `RunError::EmptyReduction`, because no summation tree has zero leaves.

## How to test

`cargo test -p calc-exec-cpu`

Each correctly rounded operation has a test on the special value that decides its IEEE behaviour. Reduce tests check both tree shapes on values where the shapes give different sums. The shared cases of `calc-conformance` also run against this backend in that crate's tests.
