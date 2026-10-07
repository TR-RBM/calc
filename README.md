# calc

[![CI](https://github.com/TR-RBM/calc/actions/workflows/ci.yml/badge.svg)](https://github.com/TR-RBM/calc/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/TR-RBM/calc)](https://github.com/TR-RBM/calc/releases/latest)
[![Code: Apache-2.0](https://img.shields.io/badge/code-Apache--2.0-blue)](LICENSE)
[![Content: CC BY-SA 4.0](https://img.shields.io/badge/content-CC%20BY--SA%204.0-lightgrey)](LICENSE-CONTENT)
[![Rust 1.98+](https://img.shields.io/badge/rust-1.98%2B-orange)](Cargo.toml)

A mathematics tool made to be used by a language model, for working on
mathematics — including mathematics that is new.

## Working something out

Which triangular numbers are perfect squares. `T(n) = n(n+1)/2 = m²` multiplies
out to `(2n+1)² − 8m² = 1`, a Pell equation. Worked in calc:

```console
$ calc --find "n=1..200" --find "m=1..200" "n*(n+1)/2 = m^2"
n = 1, m = 1
n = 8, m = 6
n = 49, m = 35
```

Forty thousand candidates. That is a search, so it is evidence and calc says
so. The argument needs two claims that a search cannot establish:

```console
$ calc --identity "(2*n+1)^2 - 8*(n*(n+1)/2) = 1"
holds for every value

$ calc --identity "(3*x+8*y)^2 - 8*(x+3*y)^2 = x^2 - 8*y^2"
holds for every value
```

The first turns the question into a Pell equation. The second says the map
`(x, y) ↦ (3x + 8y, x + 3y)` preserves `x² − 8y²`, so it sends a solution to a
solution. **No search establishes that second line.** `--find` over any range
would have been evidence where this is a proof, and knowing which of the two
you have is the whole of working something out rather than guessing it.

It is also less than the argument needs, and calc is where that shows. To get
infinitely many solutions the new pair has to differ from the old, which needs
`3x + 8y > x` on positive pairs — an inequality over a domain, and calc has no
way to say "on positive pairs" at all:

```console
$ calc --identity "3*x+8*y > x"
not an identity: it fails at x = 0, y = 0

$ calc --find "x=1..40" --find "y=1..40" --counter "3*x+8*y > x"
nothing in the range makes it fail
```

The refutation is correct and useless: the claim was never meant at the origin.
The search below it is evidence and not the step. So the growth is the one
place in this argument calc did not carry, and it says so twice rather than
once looking like a proof.

The powers of `3 + √8` come back exactly, and `√8` normalises to `2√2` without
being asked:

```console
$ printf '(3+sqrt(8))^2\n(3+sqrt(8))^3\n(3+sqrt(8))^4\n' | calc --batch --terse
17 + 12 * sqrt(2) exact
99 + 70 * sqrt(2) exact
577 + 408 * sqrt(2) exact
```

## Why a model needs one

A language model always produces an answer. That is what it is for, and it is
why it cannot be the thing that checks it. In mathematics that is already known
the cost is a wrong line. In mathematics that is new it is worse: an argument
built on a step nobody tested is not slower than one that was tested, it is
worthless, and it looks exactly the same.

calc is built the other way round.

**Every answer says which kind it is.** `exact` means no rounding happened. A
machine float says so, and the line below says by how much.

**An ambiguity is named, never resolved.** Where a written form has two
readings, calc says so and says what to write instead:

```console
$ calc "200 + 15%"
error: 200 + 15% can mean 200 + (15%) or 200 + 200 * 15%; write the one you
mean, at column 1
```

This is not pedantry, and it is not our opinion. Two mature calculators on the
same machine answer that input `230` and `200.15` respectively, neither saying
that a choice was made. Fifteen per cent, silently, in whichever direction the
parser happened to go.

**`undecided` is a verdict.** It is never read as true and never as false. It
is what a tool says when it does not know, and it exists so that nothing has to
pretend.

```console
$ calc --identity "(x+1)^2 = x^2 + 2*x + 1"
holds for every value

$ calc --identity "sqrt(x^2) = x"
not an identity: it fails at x = -1

$ calc --identity "sin(x)^2 + cos(x)^2 = 1"
undecided: calc could not bring the difference to zero by algebra alone, and
the search for a counterexample stopped at x = 1, where the claim could not be
decided
```

The third is true. calc does not know it, treats `sin` and `cos` as two
unrelated atoms, and says so rather than producing something that reads like an
answer. The second is a proof: a counterexample settles it.

## Exact by default

```console
$ calc "1/7" --terse
1/7 exact

$ calc "1/3 + 1/6" --terse
1/2 exact

$ calc "3 kg * 9.81 m/s^2" --terse
29.43 N exact

$ calc "integral(x^2, x, 0, 1)" --terse
1/3 exact
```

A fraction is answered as a fraction. Turning `1/3` into `0.333…` is a change
of form, not a simplification, and an answer in a form nobody asked for is an
answer to a different question.

Where no exact value exists, the approximation is bounded rather than hoped at:

```console
$ calc "sqrt(2)" --enclose 12
r1  sqrt(2)
  significant digits  12
  encloses            the exact value
  lower bound         1.41421356237
  upper bound         1.41421356238
  width               1e-11
  precision           reached
```

And a truncation carries what it dropped, so nothing is lost silently:

```console
$ calc "1/7" --digits 6
r1  1/7
  decimal places  6
  digits of       the exact value
  truncated       0.142857
  remainder       1/7000000
  expansion       recurring from place 1, period length 6
```

Measured values propagate their uncertainty, with the coverage factor and the
correlation assumption named:

```console
$ calc "(9.81 +- 0.02) m/s^2 * 2 s"
r1  (9.81 +- 0.02) m/s^2 * 2 s
  value        19.620 m/s
  uncertainty  ± 0.040 m/s, standard uncertainty, coverage factor k = 1
  propagation  first order from 1 uncorrelated input
  number       derived from 1 measured input
  computed     exactly, without rounding
  run time     103 µs, measured on this run
  concept      none in the concept set matches
```

## Talking to it from a program

A single call answers in JSON, with identifiers a program branches on rather
than prose it has to parse:

```console
$ calc "x^2 = 4" --solve-for --json
{"equation": "x^2 = 4", "solutions": ["-2", "2"]}

$ calc "x^3 = 2" --solve-for --json
{"equation": "x^3 = 2", "refused": "roots_not_rational", "detail": "3"}
```

A refusal is typed too. `roots_not_rational` does not change when the display
language does: the same call under `--locale de` returns the same bytes, while
the text form goes from `1/2 exact` to `1/2 exakt`.

In a batch, every outcome is one object on one line — a result, an error and a
line that would not parse alike — so a caller reads the stream line by line and
never has to find where an object ended.

Lines in a batch can refer to each other, so a recurrence runs in one pass.
Here `n(k+1) = 6n(k) − n(k−1) + 2`, the recurrence behind the Pell solutions.
The first two lines are the seeds, `n(k)` then `n(k−1)`, in that order and not
in time order, because each later line names them as `r1` and `r2`:

```console
$ printf '8\n1\n6*r1 - r2 + 2\n6*r3 - r1 + 2\n6*r4 - r3 + 2\nr5*(r5+1)/2\nsqrt(r6)\n' | calc --batch --terse
8 exact
1 exact
49 exact
288 exact
1681 exact
1413721 exact
1189 exact
```

`1413721` is a triangular number and `1189²` is it.

A definition in the batch reaches the question asked below it, under `--check`,
`--identity`, `--expand` and `--solve-for` alike:

```console
$ printf 'p(x) = x^2 - 5*x + 6\np(x) = (x-2)*(x-3)\n' | calc --batch --identity
defines  p(x) = x^2 - 5*x + 6
holds for every value
```

Where a call asks for a verdict, the exit status is the verdict, so a script
branches without reading a word:

```console
$ calc --identity "(x+1)^2 = x^2 + 2*x + 1"; echo $?
holds for every value
0

$ calc --identity "(x+1)^2 = x^2 + 1"; echo $?
not an identity: the two sides are not equal as polynomials in their names
1

$ calc --identity "sin(x)^2 + cos(x)^2 = 1"; echo $?
undecided: calc could not bring the difference to zero by algebra alone, and the search for a counterexample stopped at x = 1, where the claim could not be decided
4

$ calc --identity "((("; echo $?
not a relation  (((
3
```

The status says what answer the call got: 0 the claim holds, 1 calc proved it
false, 4 it could not decide, 3 what arrived was not a claim it can judge. A
call that asks for a value rather than a verdict keeps 1 for every way of not
getting one, because there is one way to have an answer and many not to.

## What it does not do

calc is not a computer algebra system and does not try to be. A free name is
carried through a line and answered in its normal form, and every real
root of a polynomial is an exact number, `rootof(x^3 - 2, x, 1)`, which
`--solve-for` answers with, and a list of linear equations is solved as
a system. A rational function has an antiderivative by partial
fractions; an integrand with `sin`, `exp` or `ln` has none yet, and
there is no system of equations that is not linear. `taylor(sin(x), x, 0, 5)`
answers the Taylor polynomial, and a limit of a quotient of `sin`, `cos`,
`exp`, `ln` and the like is taken through its derivatives. `calc asm`
counts the instructions and memory accesses of x86-64 assembly exactly, as a
formula in the registers a function receives, and says why it gives no cycle
count on a core that runs out of order. `calc help language` lists
every construct the input accepts.

It is also not yet the thing the first sentence of this file says it is. What
is missing showed in working the Pell equation above:

- **A name holds a polynomial, and a line answers in it.**
  `p = x^2 - 5*x + 6` defines `p`, and `diff(p*(x+1), x)` answers
  `3 * x^2 - 8 * x + 1`. A closed part is evaluated first, so
  `x + sin(pi/6)` answers `x + 1 / 2`; a quotient by a free name is not yet
  reduced, so `1/x + 1/x` answers `2 * (1 / x)`.
- **The normal form carries square roots, and `--identity` every other root.**
  `--expand "(3+sqrt(8))^2"` answers `17 + 12 * sqrt(2)`, as evaluating the
  line does, and `(x+sqrt(2))^2 = x^2 + 2*sqrt(2)*x + 2` holds for every
  value. A cube root and any other rational power is reduced by the polynomial
  it satisfies, so `(2^(1/3))^3 = 2` holds.
- **An exact algebraic value cannot be taken apart.** Nothing asks for the
  rational part of `17 + 12 * sqrt(2)`.

None of these made an answer wrong. Every number in the Pell attempt was exact
and correct. They are the difference between a tool that decides a claim and
one that carries an argument, and that difference is the work in front of us.

## Building

Stable Rust. One binary, no runtime dependencies:

```console
$ cargo build --release -p calc-cli
```

The binary is `target/release/calc`. `tools/release` produces a statically
linked one that runs on any Linux of the same architecture.

`calc --version` reports the version, the commit it was built from, whether
that tree had uncommitted changes, the target, and the session file format it
reads and writes. Two builds calling themselves `calc` can disagree about an
answer; this is how you tell which one you asked.

## Documentation

- `calc --help` — every option
- `calc help language` — every construct the input language accepts, with an example
- the `README.md` beside each crate under `crates/` — what the crate does and how to test it

Code is licensed Apache-2.0, see [LICENSE](LICENSE).
Learning content is licensed CC-BY-SA 4.0, see [LICENSE-CONTENT](LICENSE-CONTENT).

## Checks

`tools/check` runs the pre-merge checks, cheapest first:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

The clippy and test lines above are canonical: `tools/check` reads them from this section instead of holding its own copy, so the two cannot drift apart. It reads them from this section alone, and stops with a message naming what it looked for if the section is gone or holds no such line, rather than taking an indented cargo line from somewhere else in the file. In the development repository two more checks follow: one of the cross-references between code and its design records, and one of the tree that is published.

Every check prints one line, whether it ran or not: `ok`, `FAILED`, or `not run` for a check after an earlier one already failed. One line per check always appears, so an absent line is never mistaken for a pass. The full output of each check is kept under `target/check-logs/`.
