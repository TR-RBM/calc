# calc-numbers

## What it does

Holds the number values of calc. `Number` is one of `Integer`, `Rational`, `F32` and `F64`. Two machine numbers are the same number only when their bits are equal.

`Integer` is an arbitrary precision integer. It adds, subtracts, multiplies, negates, raises to a power, divides with remainder in the Euclidean sense and computes the greatest common divisor. `bit_length` is the number of bits of its magnitude, 0 for zero. `Rational` is a reduced fraction with a denominator greater than 1. It is only built through `Number::fraction`, which returns an `Integer` when the denominator reduces to 1.

`Number` does exact arithmetic with `add_exact`, `sub_exact`, `mul_exact`, `div_exact` and `negate_exact`. Each returns `MachineOperand` when an operand is `F32` or `F64` and `DivisionByZero` for a zero divisor.

`round_to_f64_ties_even` and `round_to_f32_ties_even` round any number to the nearest machine value with ties to even. Exact values round correctly, including overflow to infinity, subnormal results and signed zero. An exact zero becomes `+0`. `F32` widens to `F64` without change of value. `F64` narrows to `F32` by the same rounding. A NaN keeps its sign, becomes quiet and keeps the high bits of its payload.

`to_exact` returns the exact value of a finite machine number as `Integer` or `Rational`. `-0` becomes the integer zero. Infinities and NaN return `NotFinite`.

`minimum_f32`, `maximum_f32`, `minimum_f64` and `maximum_f64` are the IEEE 754-2019 `minimum` and `maximum`, which every backend must match. A NaN operand gives a quiet NaN, the left one when both are NaN, and `-0` is less than `+0`.

`exp_f64`, `ln_f64`, `sqrt_f64`, `pow_f64`, `sin_f64`, `cos_f64`, `tan_f64`, `asin_f64`, `acos_f64`, `atan_f64` and `atan2_f64` are the reference implementations of the elementary functions on `F64` that every backend is checked against. Each one is correctly rounded to nearest with ties to even. The rounding error bound is the same for every function and every finite result: the absolute difference between the result and the exact value is at most half a unit in the last place of the result. `elementary_rounding_error_bound_f64` returns that bound as an exact number. For zero and subnormal results the unit is the smallest subnormal. A non-finite result has no bound, and the function returns `None`.

| Function | Bound | Method |
|---|---|---|
| `exp_f64` | half an ulp | Reduction by multiples of ln 2, scaling by 2^-16, Taylor series, repeated squaring |
| `ln_f64` | half an ulp | Mantissa in [1/√2, √2), series of the inverse hyperbolic tangent, plus exponent times ln 2 |
| `sqrt_f64` | half an ulp | IEEE square root, which is correctly rounded |
| `pow_f64` | half an ulp | Exact rational result where one exists, otherwise exp of exponent times ln of the base |
| `sin_f64`, `cos_f64` | half an ulp | Reduction by multiples of π/2, Taylor series |
| `tan_f64` | half an ulp | Quotient of the sine and cosine intervals |
| `atan_f64` | half an ulp | Four argument halvings, alternating series |
| `asin_f64` | half an ulp | atan of x/√(1−x²) |
| `acos_f64` | half an ulp | 2·atan of √((1−x)/(1+x)) |
| `atan2_f64` | half an ulp | atan of the exact quotient, adjusted by π in the left half plane |

Each function except `sqrt_f64` also exists as `checked_exp_f64`, `checked_ln_f64` and so on, which return `Result<f64, ElementaryError>`. The plain functions return the checked result, or the positive quiet NaN when the checked function reports an error.

### Why half an ulp holds

The bound is proven by construction for every argument. No exhaustive check is needed, and none would be possible for `f64`.

Enclosure. Every non-trivial value is computed as a `Ball`: integers `c` and `r` at a precision `P`, standing for the interval from `(c − r)·2^−P` to `(c + r)·2^−P`. The invariant is that the exact value of the quantity lies in that interval. Every operation keeps it.

- An exact integer or dyadic number has `r = 0`. A rational `n/d` has `c` the nearest integer to `n·2^P/d` and `r = 1`.
- Addition and subtraction add the centres and add the radii.
- Multiplication: `(c1 + e1)(c2 + e2) = c1·c2 + c1·e2 + c2·e1 + e1·e2` with `|ei| ≤ ri`. The centre is `c1·c2/2^P` rounded, which is at most half a unit off, and the radius is `(|c1|·r2 + |c2|·r1 + r1·r2)/2^P` rounded up, plus 1.
- Division by a positive integer rounds the centre and takes the radius divided and rounded up, plus 1. Scaling by `2^k` is exact for `k ≥ 0` and a division by `2^−k` otherwise.
- Division by a ball needs `|c2| > r2`. The deviation from `2^P·c1/c2` is at most `2^P·(r1·|c2| + |c1|·r2)/(|c2|·(|c2| − r2))`, rounded up, plus 1.
- The square root takes the integer square root of the lower end times `2^P`, rounded down, and of the upper end times `2^P`, rounded up. The square root is monotone.

Series. Every series stops after a number of terms for which the omitted tail is below one unit `2^−P`, and the radius is widened by one unit.

- ln 2 = 2·artanh(1/3). Each term is at most `3^−(2k+1)`. After `N ≥ (P + 2)/3` terms the geometric tail is below one unit.
- π = 16·atan(1/5) − 4·atan(1/239). Both series alternate with decreasing terms, so each tail is at most its first omitted term. The term counts `(P + 6)/4` and `(P + 6)/14` make that term below one unit.
- exp. The argument is reduced by `k·ln 2` to `|r| ≤ ln 2/2` plus the radius and scaled by `2^−16`, so the `n`-th Taylor term is below `2^−17n`. After `(P + 2)/17 + 2` terms the tail is below two times the next term and below one unit. Squaring sixteen times and scaling by `2^k` use the operations above.
- ln. The mantissa lies in [1/√2, √2], so `u = (m − 1)/(m + 1)` has `|u| < 1/4`, and after `P/4 + 2` terms the tail `u^(2N+3)/(1 − u²)` is below one unit.
- sin and cos. The argument is reduced by a multiple of π/2 to `|R| ≤ π/4` plus the radius, below 1. The terms are at most `1/n!` and alternate with decreasing magnitude, so the tail is at most `1/(m + 2)!` for the first `m` with `m! > 2^(P+2)`.
- sin and cos first try a fast phase (`fixed_trig`), after Ziv's two-phase method. This is for arguments whose magnitude lies between 2⁻³⁰ and 2²⁰.
  - **Reduction.** The argument is reduced exactly by k·π/2, with π/2 held to 189 bits, into signed fixed point with 124 fraction bits.
  - **Series.** The Taylor series of sine and cosine is evaluated there in 128-bit integers, up to the terms in 1/31! and 1/32!.
  - **Error bound.** Every truncation errs by less than one unit, and the whole result by less than 17 units. The bound used is 64 units.
  - **Rounding test.** If the nearest `f64` of both ends of that range is the same, it is the correctly rounded result. Otherwise the argument goes to the series above.
  - **Same bits.** Correct rounding is unique, so the fast phase changes no bit, only the time.
- atan. Four halvings with `atan x = 2·atan(x/(1 + √(1 + x²)))` bring the argument below tan(π/32) < 1/8. The series alternates with decreasing terms, and after `P/6 + 2` terms the first omitted term is below one unit.

Rounding decision. The functions compute the correctly rounded value by Ziv's strategy. Rounding to nearest is monotone. When both ends of the ball round to the same `f64` value `v`, every real number between them rounds to `v`, and so does the exact value. `v` is then the correctly rounded result, and its distance to the exact value is at most half the gap to the neighbouring machine number, which is at most half an ulp of `v`. Exact results, such as the special values and the rational powers, are rounded with `round_to_f64_ties_even`, which is correct rounding of an exact number.

Precision limit. When the ends round differently, the precision doubles and the value is computed again. The loop, and the overflow probe of `pow_f64`, stop at `2^16` bits with `ElementaryError::PrecisionLimitReached`. Termination. An enclosure that contains a rounding boundary in its interior cannot decide the rounding, and every rounding boundary is rational. So every rational result is returned exactly before the loop, and the loop ends for every irrational result, because the radius tends to zero as the precision doubles.

- `exp_f64`, `ln_f64`, `sin_f64` and `cos_f64`: their results at floating-point arguments other than the special cases are irrational, so the loop ends for every argument.
- `tan_f64`, `atan_f64`, `asin_f64`, `acos_f64`, `atan2_f64` and `pow_f64`: the corpus has no block for the irrationality of their non-rational results, which rests on further theorems such as Gelfond–Schneider. Termination is argued but not covered by the corpus for these six.

The limit stays for all eleven functions. It keeps an argument that a gap in the rational cases or in the argument above misses from hanging the process. A result at the limit is an error, never a value, so the half-ulp bound is never attached to an undecided value. No argument used in the tests or in the 33000-argument comparison reaches the limit.

Tests check the invariant for each ball operation against exact rationals, check that the balls for π, ln 2, exp, ln, sin, cos and atan contain 60-digit references, and check that the loop returns the typed error at a small limit for a value centred on a rounding midpoint.

Special values follow IEEE 754-2019: a NaN argument gives the same NaN made quiet, an invalid operation gives the positive quiet NaN `0x7ff8000000000000`, and zeros, infinities and signs follow the recommended operations of section 9.2.

The functions are slow compared with a platform maths library, typically 50 µs to 2 ms per call in a release build. They define the expected values and bounds for the backends. They are not meant for bulk evaluation.

`Interval` is an enclosure with `f64` ends for rigorous error bounds. Every operation rounds its ends outward by one machine number, so the exact result of the operation on any values inside the operands lies inside the result. The elementary functions use the correctly rounded functions above at the ends and widen by one machine number, which covers the half-ulp error of those functions. `sin` and `cos` add the extreme values 1 and -1 when a multiple of π/2 may lie inside the interval, and return [-1, 1] for intervals wider than 7 or beyond 10^15. `tan` fails across a possible pole, `atan2` across the branch cut or at the origin, and `pow` for a base interval that is not positive, except a negative point base with an integer point exponent. An operation without an enclosure returns `None`. Comparisons return a `Truth` of `True`, `False` or `Unknown`. `distance_bound` gives an upper bound on the distance from a value to every point of the interval.

`exp_f32`, `ln_f32`, `sqrt_f32`, `pow_f32`, `sin_f32`, `cos_f32`, `tan_f32`, `asin_f32`, `acos_f32`, `atan_f32` and `atan2_f32` are the reference implementations on `F32`. They compute in `f32` arithmetic only: addition, subtraction, multiplication, division, square root, `floor`, `trunc`, `round_ties_even`, `%` and comparisons. No `f64` value and no fused multiply-add takes part. Integers appear only as loop counters, table indices and exponents of exact power-of-two scaling. So a backend in the `F32` domain can run the same algorithm without a higher intermediate precision.

The algorithms use double-word arithmetic: a value is a pair of `f32` whose exact sum carries about 46 bits. Sums and products are split exactly with the error-free transformations TwoSum, FastTwoSum and Dekker's TwoProduct with the splitter 4097. The double-word sum, product and quotient follow Joldes, Muller and Popescu (2017), whose proven relative error bounds are at most 15·2^-48, below 2^-44. The double-word square root applies one Newton correction to the `f32` square root. Constants are double-word values of ln 2 and multiples of π, and twelve 24-bit chunks of 2/π for the argument reduction of the trigonometric functions. Unit tests check every constant against the interval code of `series.rs`.

| Function | Method |
|---|---|
| `exp_f32` | Reduction by multiples of ln 2, Taylor series with 14 terms, exact scaling by the power of two, with integer rounding in the subnormal range |
| `ln_f32` | Mantissa in [1/√2, √2], series of the inverse hyperbolic tangent with 10 terms, plus exponent times ln 2 |
| `sqrt_f32` | IEEE square root |
| `pow_f32` | exp of exponent times the double-word logarithm of the base, after the IEEE special cases |
| `sin_f32`, `cos_f32`, `tan_f32` | Exact product of the argument with chunks of 2/π summed as an expansion modulo 4, remainder times π/2, Taylor series with 11 terms |
| `atan_f32` | Reciprocal for arguments above 1, four argument halvings, alternating series with 5 terms |
| `asin_f32`, `acos_f32` | atan of x/√(1−x²) and 2·atan of √((1−x)/(1+x)) |
| `atan2_f32` | Exact power-of-two scaling of large or small operands, atan of the smaller over the larger operand, adjusted by π/2 and π |

The rounding error bound of every `F32` elementary function is one unit in the last place of the result: for every finite result, the absolute difference between the result and the exact value is at most one ulp of the result. `elementary_rounding_error_bound_f32` returns that bound as an exact number, with the smallest subnormal as the unit for zero and subnormal results, and `None` for a non-finite result. An estimate that adds the per-operation bounds over the at most about 100 operations of one evaluation gives an accumulated relative error below 2^-37 before the final rounding, so the design error is half an ulp plus less than 2^-13 ulp. The stated bound of one ulp leaves a margin of more than 2^12 over that estimate. `sqrt_f32` is correctly rounded.

The one-variable `F32` functions were run on all 2^32 arguments and compared with the platform `f64` function rounded to `f32`. Every argument where the two results differ was decided with a 150-digit evaluation in Python's `decimal` module. The table gives the number of such arguments, how many of them this implementation rounds to the wrong side of a near-midpoint, and the largest exact error over those decided arguments, in ulps of the result. On all other arguments the result equals the rounded `f64` function, whose documented maximum error in glibc is about one `f64` ulp, which is 2^-29 ulp of an `f32`, so their error exceeds half an ulp by less than about that. No argument comes near the stated bound of one ulp.

| Function | Arguments differing from rounded f64 | Misrounded here | Largest exact error |
|---|---|---|---|
| `sqrt_f32` | 0 | 0 | correctly rounded |
| `exp_f32` | 26 | 26 | 0.500001067 |
| `ln_f32` | 56 | 56 | 0.500000069 |
| `sin_f32` | 100 | 98 | 0.500000068 |
| `cos_f32` | 88 | 88 | 0.500000050 |
| `tan_f32` | 66 | 66 | 0.500000121 |
| `asin_f32` | 22 | 22 | 0.500000222 |
| `acos_f32` | 26 | 26 | 0.500000193 |
| `atan_f32` | 40 | 40 | 0.500000158 |

So the `F32` functions are not correctly rounded: 422 of the 3.4·10^10 pairs of function and argument land on the wrong side of a midpoint they are closer to than 2^-19 ulp; these are the hard cases of the table maker's dilemma. `pow_f32` and `atan2_f32` cannot be checked exhaustively. On 6000 generated argument pairs they agreed with a correctly rounded reference in every case.

Arguments below 6·10^-5 in magnitude return the argument for `sin_f32`, `tan_f32`, `asin_f32` and `atan_f32`, because the neglected cubic term is below 2^-5 ulp. The special values follow IEEE 754-2019 as for the `F64` functions, with the positive quiet NaN `0x7fc00000` for invalid operations.

Decimal places. `decimal_digits(value, places)` truncates an exact or finite machine value toward zero after `places` decimal places, up to `DECIMAL_PLACES_LIMIT` (78913). It returns the truncated `digits` as an exact decimal, the exact `remainder` (value minus digits, with the sign of the value and a magnitude below `10^-places`), whether the value `terminates` within the places, whether its whole expansion is `finite`, and the `period` of an infinite expansion. For a reduced denominator `2^a · 5^b · m` with `m` coprime to 10, the period starts at place `max(a, b) + 1` and has the multiplicative order of 10 modulo `m` as its length, computed by exact residues. It is absent when it would end beyond the limit. A machine value gives the digits of the binary number it holds, and NaN or an infinity is `NotFinite`. `round_to_decimal_places(value, places, DecimalRounding::TiesToEven)` rounds to the nearest multiple of `10^-places`, a tie to the even last digit as machine rounding does, and returns the rule, the rounded decimal and the exact error, value minus rounded.

Decimal enclosures. `RealBall` is a public ball of the arbitrary precision arithmetic, created from an `EnclosureStep` that fixes its fraction bits, the bit budget and the cancellation closure: exact numbers and finite machine numbers (as the binary value they hold), `pi`, `e`, sum, difference, product, quotient, negation, absolute value, square root, integer power, `exp`, `ln`, `sin`, `cos`, `tan` and `atan`. Each operation returns `BallError::NoEnclosure` where it cannot prove an enclosure at this precision (a divisor or logarithm argument not surely away from zero, a square root argument not surely nonnegative, an argument of `exp`, `sin`, `cos` or `atan` wider than 1/16), `OverBudget` with the integer bits it would need where a center, integer and fraction bits together, would exceed the budget (checked before an exact value is scaled, before a product, quotient or exponential is formed, and after every result), and `Cancelled` when the closure answers true between the terms of a series. `sin` and `cos` halve their reduced argument about `sqrt(p)/2` times and recover by the double-angle formulas at a working precision raised by twice that many bits plus 16, never above the budget less the integer bits of the argument, so the budget holds inside the series too. `enclose_to_significant_digits(n, budget_bits, is_cancelled, evaluate)` is the Ziv loop. It probes at 64 fraction bits, then asks for the precision the request needs, `3.4 · (n + d) + 32` bits for a value `d` decades below 1 and 3.4 bits less per decade above 1, and doubles from there. The ceiling of the fraction bits is the budget minus the integer bits of the best interval and of any over-budget report, and an over-budget step is retried once at that ceiling. It keeps the intersection of every proven interval and rounds it outward to `n` significant digits, the lower end down and the upper end up on the grid of its own decade. The outcome has `lower`, `upper`, `width` and `reached`: `reached` is true when the ends are equal, or when neither is zero and the width is at most `10^(e − n + 1)` with `e` the decade of the end of smaller magnitude. It stops at the first reached interval, and at the ceiling returns the tightest proven interval unreached. With no proven interval it fails with `NoEnclosure` or, when the numbers outgrew the budget, `OverBudget`, both naming the budget. It checks `is_cancelled` before every step. `n` runs from 1 to `SIGNIFICANT_DIGITS_LIMIT` (5000), and `ENCLOSURE_BUDGET_BITS` is `3.4 · 5000 + 32` = 17032 bits for every request. Measured in a release build on the development host: `sin(1)`, `cos(1)` 0.9 s, `tan(1)` 0.6 s, `ln(2)` 0.6 s, `atan(1)` 1.5 s, `e` 0.7 s, `pi` 0.5 s, `exp(ln(2)/3)` 0.7 s, all reached at 5000 digits; `sin(pi)` unreached at 5000 digits 2.0 s and at 5 digits 0.6 s; `exp(-150000)` at 20 digits unreached in 0.9 s; `exp(100000)` and `10^70000 · sin(1)` over budget in under 15 ms. The digits view keeps its 78913 places. `enclose_exact` rounds an exact value outward the same way without a loop. `round_to_significant_digits` rounds an exact or finite machine value to nearest on the same grid with ties to even, `round_half_away_to_significant_digits` rounds a tie away from zero as schools do, `at_least_one` runs the budgeted loop until the enclosure lies wholly at or above 1 or wholly below it and gives None where it cannot separate them, `correctly_rounded_to_significant_digits` runs the same budgeted loop until both ends of the enclosure round to nearest alike, and `smallest_f64_at_least` gives the smallest `f64` not below an exact value, for bounds that must round upward.

Power-of-ten form. `power_of_ten_text(digits, exponent, separator)` writes the exponent form both a tick label and a reading use: the first significant digit, the separator the caller passes, the remaining digits where there are any, `POWER_OF_TEN_MARK` and the exponent in decimal with a minus where it is negative. A leading minus in the digits stays in front. It wants at least one digit: with none it would write a mark and an exponent with no number, which neither caller can ask for, a tick label returning early for a zero significand and a reading writing a zero plainly. The digits are the caller's: a tick label strips trailing zeros and a reading keeps four significant digits, and both use the same power-of-ten form, which is why the form itself lives here and not in either of them.

The crate has no dependencies. Word size conversions with `as` live only in `word_conversion.rs`.

`enclose_between` rounds a pair of exact bounds outward to a number of significant digits, as `enclose_exact` does for one value; the command line uses it for the enclosures of `--ode`.

`Interval`'s add, sub, mul, div and sqrt round to nearest and widen each end by one ulp, except where the end is exact in IEEE arithmetic by a zero operand or by scaling with a power of two into a normal result; there it is kept as it is.

`atan_f64` and `atan2_f64` first try a double-word evaluation: a table of atan(i/64), a short series, and a rounding test that returns the result only when its proven error bound of 2⁻⁷⁵ cannot change the rounding; otherwise they take the ball path, so every answer is the same correctly rounded double the ball path gives.

`sin_f64` and `cos_f64` first try a double-word evaluation for 2⁻³⁰⁰ ≤ |x| ≤ 2²⁰: Cody–Waite reduction with π/2 in three parts, a table of sin and cos of j/64, short series, and the arctangent's rounding test with the reduction's absolute error added; when it does not decide they take a fixed-point phase and then the ball path, so every answer is the same correctly rounded double the ball path gives. `binary64_words` holds the double-word operations and rounding tests the arctangent, sine and cosine share.

## How to test

`cargo test -p calc-numbers`

The bounded noncancellable reuse of ln(2) is checked against uncached Ball enclosures at identical precisions, the uncached exponential enclosure and output bits for both exact f64 and radius-bearing power arguments, and the cancellable series path.

Rounding tests compare against the correctly rounded decimal parser of the Rust standard library over several thousand generated decimals, and against exact decimal expansions for narrowing. Elementary function tests check the result against 60-digit reference values within the stated bound, and check the IEEE special values. The reference values were computed with Python's `decimal` module, using its correctly rounded `exp`, `ln`, `sqrt` and power, and independent Newton and Taylor implementations for the trigonometric functions. The same oracle agreed bit for bit with `exp_f64`, `ln_f64`, `sin_f64`, `cos_f64`, `tan_f64`, `asin_f64`, `acos_f64`, `atan_f64` and `pow_f64` on 33000 generated arguments. Integer tests compare against `i128` and `u128` arithmetic and check that long division reconstructs the dividend.
