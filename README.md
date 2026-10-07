# calc

[![CI](https://github.com/TR-RBM/calc/actions/workflows/ci.yml/badge.svg)](https://github.com/TR-RBM/calc/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/TR-RBM/calc)](https://github.com/TR-RBM/calc/releases/latest)
[![Code: Apache-2.0](https://img.shields.io/badge/code-Apache--2.0-blue)](LICENSE)
[![Content: CC BY-SA 4.0](https://img.shields.io/badge/content-CC%20BY--SA%204.0-lightgrey)](LICENSE-CONTENT)
[![Rust 1.98+](https://img.shields.io/badge/rust-1.98%2B-orange)](Cargo.toml)

A mathematics tool made to be used by a language model, for working on
mathematics.

**calc is an open source calculator for the command line that keeps every result exact as long as the mathematics allows, and says what a result is where it does not.** A language model, a script and a person at a terminal use the same command; it answers as text or as JSON.

A language model always produces an answer, which is why it cannot be the thing that checks one. calc is built the other way round:

- **Exact by default.** `1/3 + 1/6` is `1/2`, not `0.5`, and `sqrt(8)` is `2 * sqrt(2)`. A fraction is answered as a fraction.
- **Bounded where nothing is exact.** A decimal comes with the remainder that makes it exact, an interval with proven bounds, and a machine float with the format it was computed in and the size of its rounding error.
- **A verdict, not a guess.** A claim holds, fails, or is `undecided`, and the three are never confused. A search over a range is reported as evidence, an identity proved by algebra as a proof.
- **Ambiguity named, never resolved.** Where a written form has two readings, calc says which two and what to write instead.
- **Units and uncertainty carried.** A unit is computed with the value and checked, and a measured value carries its uncertainty through the calculation.
- **Built for programs.** JSON with identifiers a program branches on, refusals that are typed, and an exit status that is the verdict.

calc is built and tested on x86-64 Linux. It is not a computer algebra system; what it does not do yet is listed [below](#what-it-does-not-do-yet).

## A short example

Which triangular numbers are perfect squares? `n(n+1)/2 = m²` is a Pell equation in disguise. A search finds the first solutions:

```console
$ calc --find "n=1..200" --find "m=1..200" "n*(n+1)/2 = m^2"
n = 1, m = 1
n = 8, m = 6
n = 49, m = 35
```

That is evidence, not an argument. The argument needs two claims no search can establish, and calc proves both by algebra:

```console
$ calc --identity "(2*n+1)^2 - 8*(n*(n+1)/2) = 1"
holds for every value

$ calc --identity "(3*x+8*y)^2 - 8*(x+3*y)^2 = x^2 - 8*y^2"
holds for every value
```

The first turns the question into `x² − 8y² = 1`; the second shows that `(x, y) ↦ (3x + 8y, x + 3y)` sends a solution to a solution. Its powers come back exactly:

```console
$ printf '(3+sqrt(8))^2\n(3+sqrt(8))^3\n(3+sqrt(8))^4\n' | calc --batch --terse
17 + 12 * sqrt(2) exact
99 + 70 * sqrt(2) exact
577 + 408 * sqrt(2) exact
```

One step calc cannot carry: that the new pair is larger, `3x + 8y > x` on positive pairs. calc has no way yet to say "on positive pairs", so it refutes the claim at the origin and leaves the step open rather than looking like a proof:

```console
$ calc --identity "3*x+8*y > x"
not an identity: it fails at x = 0, y = 0
```

## What you can do

### Calculate exactly

```console
$ calc "1/3 + 1/6" --terse
1/2 exact

$ calc "integral(x^2, x, 0, 1)" --terse
1/3 exact

$ calc "sum(k^2, k, 1, 100)" --terse
338350 exact

$ calc "0.1 + 0.2" --terse
0.3 exact

$ calc "to_f64(0.1 + 0.2)" --terse
0.30000000000000004 machine
```

calc reads decimals exactly, so `0.1 + 0.2` is `0.3`; `to_f64` asks what a 64-bit float makes of it.

### Bound what is not exact

```console
$ calc "sqrt(2)" --enclose 12
r1  sqrt(2)
  significant digits  12
  encloses            the exact value
  lower bound         1.41421356237
  upper bound         1.41421356238
  width               1e-11
  precision           reached

$ calc "1/7" --digits 6
r1  1/7
  decimal places  6
  digits of       the exact value
  truncated       0.142857
  remainder       1/7000000
  expansion       recurring from place 1, period length 6
```

`--ode` encloses the solution of a system of ordinary differential equations with a proven bound, units included: `calc --ode "diff(y, t) = -y" --initial "t = 0; y = 1" --at "t = 1"`.

### Decide claims

| You want to know | Call |
|---|---|
| whether a claim holds for these values | `calc --check "2^64 - 1 = 18446744073709551615"` |
| whether it holds for every value of its names | `calc --identity "(x+1)^2 = x^2 + 2*x + 1"` |
| which whole numbers in a range make it hold, or fail | `calc --find "n=1..200" --counter "n^2 > n"` |

An identity has six answers: it holds for every value, holds wherever a denominator is not zero, is not an identity (with a counterexample), fails for every value, fails wherever defined, or is `undecided`:

```console
$ calc --identity "sqrt(x^2) = x"
not an identity: it fails at x = -1

$ calc --identity "sin(x)^2 + cos(x)^2 = 1"
undecided: calc could not bring the difference to zero by algebra alone, and
the search for a counterexample stopped at x = 1, where the claim could not be
decided
```

The second is true. calc does not know it yet and says so, rather than producing something that reads like an answer.

Where a written form is ambiguous, calc asks:

```console
$ calc "200 + 15%"
error: 200 + 15% can mean 200 + (15%) or 200 + 200 * 15%; write the one you
mean, at column 1
```

### Work with units and measured values

```console
$ calc "3 kg * 9.81 m/s^2" --terse
29.43 N exact

$ calc "100 km / 1 h" --units us-customary --terse
(781250/12573) mi/h exact

$ calc "(9.81 +- 0.02) m/s^2 * 2 s"
r1  (9.81 +- 0.02) m/s^2 * 2 s
  value        19.620 m/s
  uncertainty  ± 0.040 m/s, standard uncertainty, coverage factor k = 1
  propagation  first order from 1 uncorrelated input
  ...
```

A unit system changes the unit a value is shown in, never the value. `5 m + 3 s` is refused.

### Do algebra and calculus

```console
$ calc --expand "(x+1)^3"
x^3 + 3 * x^2 + 3 * x + 1

$ calc --solve-for "x^2 - 5*x + 6 = 0" --terse
2, 3

$ calc --factor "360/84"
2 * 3 * 5 / 7

$ calc "diff(x^3*sin(x), x)" --terse
x^3 * cos(x) + 3 * x^2 * sin(x) exact
```

Every real root of a polynomial is an exact number, written `rootof(x^3 - 2, x, 1)`; a list of linear equations is solved as a system; a rational function is integrated by partial fractions; `taylor(sin(x), x, 0, 5)` gives the Taylor polynomial, and a limit of a quotient is taken through its derivatives.

### Draw and read pictures

```console
$ calc plot "sin(x)" --output wave.png
wave.png, 800 by 600 pixels
```

`--view`, `--param` and `--size` set the axes, free parameters and image size. `calc read` reads a value off a line's picture at coordinates you type and can keep it as a new line of the session.

### Keep sessions and run batches

A `.calc` session file records every line with its value, how it was computed and how exact it is; `calc --replay` computes each line again and says whether it still gives what was stored. In a batch, lines refer to earlier ones as `r1`, `r2`, …, and a definition reaches every line below it:

```console
$ printf 'p(x) = x^2 - 5*x + 6\np(x) = (x-2)*(x-3)\n' | calc --batch --identity
defines  p(x) = x^2 - 5*x + 6
holds for every value
```

### Call it from a program

```console
$ calc "x^2 = 4" --solve-for --json
{"equation": "x^2 = 4", "solutions": ["-2", "2"]}

$ calc "x^3 = 2" --solve-for --json
{"equation": "x^3 = 2", "refused": "roots_not_rational", "detail": "3"}
```

The JSON is the same in every display language. In a batch under `--json` every outcome is one object on one line. Where a call asks for a verdict, the exit status is the verdict:

| Status | Meaning |
|---|---|
| 0 | the work was done, or the claim holds |
| 1 | the work failed, or calc proved the claim false |
| 2 | the command line was not understood |
| 3 | what was read is not something calc can judge |
| 4 | undecided, or nothing in a `--find` range matched |

### Count what algorithms do

Sorting methods such as `insertion_sort`, `merge_sort` and `selection_sort` sort a list exactly and count the comparisons and writes on that input, beside the fewest, most and average counts as formulas with their sources; `--trace` shows every step. `calc asm` counts the instructions and memory accesses of x86-64 assembly exactly, as a formula in the registers a function receives, and says why the code does not determine the cycles.

## What it does not do yet

- There is no way to state a claim on a domain, such as "for positive x".
- An integrand with `sin`, `exp` or `ln` has no antiderivative yet, and a system of equations that is not linear is not solved.
- A quotient by a free name is not reduced: `1/x + 1/x` answers `2 * (1 / x)`.
- An exact algebraic value cannot be taken apart: nothing asks for the rational part of `17 + 12 * sqrt(2)`.
- `sin` and `cos` are unrelated to the identity prover, so `sin(x)^2 + cos(x)^2 = 1` is `undecided`.

None of these makes an answer wrong; each is a claim calc leaves open.

## Installation

calc builds with stable Rust 1.98 or later into one binary with no runtime dependencies:

```sh
cargo build --release -p calc-cli
```

The binary is `target/release/calc`. `tools/release` builds a statically linked one that runs on any x86-64 Linux; the release workflow does the same for a version tag. `calc --version` names the version, the commit it was built from, whether that tree had uncommitted changes, and the target.

## Documentation

- `calc --help`: every command and option, the exit statuses and examples.
- `calc help language`: every construct the input language accepts, with an example of each.
- The `README.md` of each crate under `crates/`: what the crate does and how to test it.

## Contributing and security

How to build, test and send a change is in [CONTRIBUTING.md](CONTRIBUTING.md). Report a vulnerability privately as [SECURITY.md](SECURITY.md) describes.

## Licence

Code is licensed under Apache-2.0, see [LICENSE](LICENSE). The learning content under `content/` is licensed under CC BY-SA 4.0, see [LICENSE-CONTENT](LICENSE-CONTENT).

## Checks

`tools/check` runs the gate, cheapest first, and stops at the first failure:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

`tools/check` reads the clippy and test lines from this section, so the two cannot drift apart. Each check prints one line, `ok`, `FAILED` or `not run`, and its full output is kept under `target/check-logs/`.
