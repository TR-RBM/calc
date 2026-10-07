# calc-exec

## What it does

The execution layer interface.

- `Plan` is a validated circuit over one domain, `F32` or `F64`. Gates are inputs, constants, operations whose arguments are earlier gates, and iteration gates. `Plan::new` checks channel ranges, constant domains, arity, argument order and the scalar or boolean kind of every argument. A plan has at least one scalar output. A plan with a reduce stage has exactly one output, and its output batch has length 1.
- `PlanOp` is the closed operation enum with its class, correctly rounded or approximate, and its signature.
- `Batch` holds one contiguous column per channel, all of one domain and one length.
- An `Iterate` gate holds an `Iteration`: initial gates for its state slots, a body sub-circuit of `State`, `Outer`, `Constant` and `Operation` body gates, one `next` body gate per slot, a boolean `exit` and a `maximum_count`. `IterationState` gates read the final state. Each element tests the exit before each step and stops there, or after `maximum_count` steps. `Plan::new` checks slot kinds, body order and kinds, and every reference, with one `PlanError` or `BodyError` variant per failure.
- `Plan::unrolled` writes each iteration out as `maximum_count` body copies with a sticky done flag, freezing scalar slots with `Select` and boolean slots with `And`, `Or` and `Not`. Its outputs have the same bits as the iteration, which is the contract every backend meets.
- `escape_time_plan` builds the unrolled escape-time circuit. `escape_time_iteration_plan` builds the same picture with one iteration gate. Both give the same counts and flags.
- `Backend` and `Prepared` are the backend traits. `Capabilities` lists domains, operations, the largest batch length, the largest iteration count, 0 for a backend without iteration gates, whether approximate operations run with the `calc-numbers` reference implementations, which a plan with an approximate operation in an iteration body requires, and the cost parameters. The cost estimate counts each body times its `maximum_count`.
- `Parallelism` is the capability a backend receives at registration to split work. `SharedParallelism` is its cloneable handle, and `SequentialParallelism` runs tasks one after another on the calling thread.
- `select` picks a backend. `Preference::Only` uses exactly the named backend or fails. `Preference::Automatic` estimates setup, gate evaluations and transfers in integer nanoseconds, prefers the smallest estimate, breaks ties by registration order, and moves to the next estimate when `prepare` reports an unsupported plan. Every skipped backend is recorded with its reason.
- `lower(pool, root, spec)` turns an expression into a `Plan`. `LowerSpec` names the domain, the input symbols in channel order with their form, real or complex, and the output form. A complex input takes two channels, real part then imaginary part. A complex output gives two output channels.
  - Exact numbers are rounded once to the plan domain at the leaf. A machine number of the other domain, or a conversion to it, is `MixedMachineDomain`. A conversion to the plan domain adds no gate.
  - `pi`, `e` and `inf` become the nearest machine value, and `i` becomes the complex value 0 + 1i.
  - Each pool node becomes at most one gate or gate pair, so shared subexpressions stay shared. The written shape is kept: no gate is reordered, flattened or contracted.
  - Operators map one to one onto `PlanOp`. Complex values support `Complex`, `Neg`, `Add`, `Sub` and `Mul`, expanded into real gates as written: (a + bi)(c + di) is (a·c − b·d) + (a·d + b·c)i. Every other operator on a complex value is `ComplexNotSupported`.
  - A root `Sum` or `Product` with constant integer bounds becomes a map over the index and a reduce with the binder's shape. The index is the input channel after all inputs of the spec. The caller fills it with the index values from the lower to the upper bound, rounded to the domain.
  - `Asin`, `Acos`, `Atan`, `Factorial`, `ToExact`, the uncertainty operators, `ConvertUnit`, the temperature conversions (which `calc-core` expands before lowering), user functions, other binders, quantities and arrays do not lower and return an error that names them.

## How to test

`cargo test -p calc-exec`

Selection is tested with test backends inside `src/selection.rs`. Iteration validation and unrolling are tested in `src/plan.rs`, one test per error variant. Lowering is tested in `src/lowering.rs` with one test per node kind, per operator group and per error, by inspecting the gates of the plan. Nothing in the crate runs a plan.
