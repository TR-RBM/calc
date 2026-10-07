# rounding_oracle

## What it does

Checks the rounding error bound that each elementary function of calc-numbers claims. For an argument and a result, the oracle encloses the exact value of the function and decides whether the distance between the result and the exact value is at most the claimed bound. It uses only the public API of calc-numbers.

The claimed bound is whatever `elementary_rounding_error_bound_f32` or `elementary_rounding_error_bound_f64` returns for the result. A finite result without a claim is reported as a missing claim. An infinite result is within bound when the exact value rounds to that infinity under rounding to nearest. A NaN result is within bound only where the function is undefined or an argument is NaN. Signs of zero and NaN payloads are not checked.

The exact value is known in one of four ways. Special values follow IEEE 754-2019 section 9.2. Rational results are computed exactly with `Number`: powers of two, integer powers up to 64 and roots of perfect powers, including midpoints such as 3^34. Values beyond the overflow threshold of both formats are known to overflow. Every other value is enclosed by interval arithmetic.

The interval arithmetic has two tiers behind one trait, `Arithmetic`, so every function is written once.

- The machine tier stores `f64` endpoints and moves each endpoint one step outward after every operation. It decides almost every `f32` case and is fast enough for all 2^32 arguments. Large trigonometric arguments are reduced with a 126-bit window of 2/π per exponent, computed once from the dyadic tier.
- The dyadic tier stores endpoints as calc-numbers `Integer` values scaled by 2^precision. It decides the `f64` cases and every case the machine tier cannot decide. The oracle raises the precision from 96 bits up to 3072 bits plus an argument-dependent extra. A case that is still undecided is reported as undecided and never guessed.

The enclosures are:

| Function | Enclosure |
|---|---|
| exp | Reduction by a multiple of ln 2, division by 16, Taylor series with remainder at most twice the next term, four squarings |
| ln | Mantissa in [0.75, 1.5), series of the inverse hyperbolic tangent with remainder at most twice the next term, plus exponent times ln 2 |
| sqrt | Outward rounded square root, exact integer square root in the dyadic tier |
| sin, cos, tan | Reduction by the nearest multiple of π/2, Taylor series with the Lagrange remainder, tan as the quotient |
| atan | Four halvings z/(1+√(1+z²)), alternating series with remainder at most the next term |
| asin, acos | atan of x/√((1−x)(1+x)), twice atan of √((1−x)/(1+x)) |
| pow | exp of exponent times ln of the magnitude, after the exact cases |
| atan2 | atan of the quotient interval, adjusted by π in the left half plane |

π comes from Machin's formula and ln 2 from twice the inverse hyperbolic tangent of 1/3, both computed with 64 guard bits.

## How to test

`cargo test -p calc-numbers --test rounding_oracle` runs the unit tests of the oracle. They check the enclosures against 50-digit values of e, π, ln 2 and √2 and against identities, and show that the oracle reports results misrounded by one or two units in the last place, a wrong quadrant, a NaN for a valid argument and an infinity below the overflow threshold (E-PRF-V-002).

The runs over the functions are ignored tests, run in release:

- `cargo test --release -p calc-numbers --test rounding_oracle exhaustive_f32_shard -- --ignored --test-threads=4` checks exp, ln, sqrt, sin, cos, tan, asin, acos and atan on every `f32` argument, in 64 shards. It takes about an hour on four threads.
- `cargo test --release -p calc-numbers --test rounding_oracle sampled_ -- --ignored` checks every `f64` function and `pow_f32` and `atan2_f32` on the special values and a seeded sample uniform over bit patterns.

A failing run lists the function, the argument bits, the result bits and the verdict of up to 64 findings, and counts the rest.
