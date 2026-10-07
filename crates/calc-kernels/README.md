# calc-kernels

## What it does

Fast kernels for work a caller does per frame or per step. Each kernel is also reached through calc, which is its reference. The crate is `no_std`, depends on nothing, allocates nothing, locks nothing and does no input or output.

The first kernel is the counter-based random number generator Philox4x32-10:

- `philox4x32_10(counter, key)` is the generator itself: four 32-bit counter words and two 32-bit key words give four 32-bit output words.
- `philox4x32_10_at(seed, stream, index)` names a block by three 64-bit numbers: the key is the seed, the counter is `index + 2^64 · stream`, each split into 32-bit words with the low word first.
- `philox4x32_10_fill(seed, stream, first_index, words)` fills the caller's slice with the words of the blocks at `first_index`, `first_index + 1`, and so on, in order.

Philox is a statistical generator. It is not suitable for cryptography, and nothing here may be used where an adversary must not predict the output.

### The kernel contract, point by point, for Philox4x32-10

1. **Two layers.** The generator is integer arithmetic with one exact result, so layer 1 and layer 2 are the same function: calc answers `philox4x32_10(seed, stream, index)` by calling `philox4x32_10_at`.
2. **Where it lives.** Here, `no_std`, with no dependency.
3. **The caller's memory.** The scalar form is `philox4x32_10_at`; the batch form is `philox4x32_10_fill` over a `&mut [u32]` the caller owns. Any alignment is accepted; the slice is written and never read; a length that is not a multiple of four uses the first words of the last block. No scratch memory is needed. There is no layout to choose.
4. **Bounded work.** Ten rounds per block, a fixed count, and one block per four words of the slice. No failure is possible.
5. **Determinism.** Only 32-bit multiplications widened to 64 bits, exclusive or and wrapping addition; no floating point, so no FMA, rounding, NaN, zero sign or subnormal policy is needed. The result is the same bits on every machine. Streams are assigned by (seed, stream, index), and the index wraps from 2^64 − 1 to 0 within its stream.
6. **The bound.** Domain: every seed, stream and index in 0 to 2^64 − 1. Nothing lies outside it. Coefficients: the multipliers 0xD2511F53 and 0xCD9E8D57 and the key increments 0x9E3779B9 and 0xBB67AE85 of Salmon et al. (2011). Arithmetic: exact integer arithmetic modulo 2^32 and 2^64 as written. Error: none; the output is the definition's, bit for bit.
7. **Checks.** Scalar and batch paths give identical bits (`tests/philox.rs`). The three published known-answer vectors of Random123 are reproduced (`tests/data/`, BSD 3-Clause with its notice). An independent implementation in PARI/GP reproduces the same vectors and gives four further values, which `tests/philox.rs` reproduces bit for bit. The benchmark against a hand-written loop is `examples/philox_fill.rs`.
8. **The GPU.** There is no shader form yet.
9. **Runtime fallback.** Not applicable: no sign is decided.
10. **Tables.** None.

### A stable radix sort of 32-bit keys

- `radix_sort_u32(keys, payload, scratch_keys, scratch_payload, histogram)` sorts `keys` in increasing order and moves `payload` with them, keeping equal keys in their input order. It makes four passes over 8-bit digits, the least significant first, and skips a pass whose digit is the same for every key, which is what a narrow range of keys costs nothing for. It returns how many passes it made and skipped.
- `radix_sort_f32(keys, payload, ordered, scratch_keys, scratch_payload, histogram)` sorts `f32` keys in IEEE 754 totalOrder: −NaN, −∞, the negative numbers, −0, +0, the positive numbers, +∞, +NaN, each NaN by its bits. `total_order_bits` and `from_total_order_bits` map an `f32` to and from the `u32` whose order is that order. Every bit pattern comes back unchanged.

The kernel contract, point by point:

1. **Two layers.** calc's `radix_sort` is the exact form on whole numbers; this kernel is the fast one for 32-bit keys. Its tests compare it with the standard library's stable sort by key and, for `f32`, by `f32::total_cmp`, which is totalOrder.
2. **Where it lives.** Here, `no_std`, with no dependency.
3. **The caller's memory.** Keys, payload, two scratch slices of the same length and a histogram of `RADIX_BUCKETS` counters, all the caller's. Any alignment; `keys` and `payload` are read and written, the scratch and histogram overwritten; the slices may not alias, which the borrow rules already ensure. There is no tail: every length is sorted whole.
4. **Bounded work.** At most four passes of one count and one scatter each, and one copy back; no allocation. Slices of different lengths are refused with `RadixRefusal::LengthsDiffer` before anything is written, and more than 2^32 − 1 keys with `TooLong`.
5. **Determinism.** Integer operations only; the same bits on every machine for every input, NaN included, since the order is on bits.
6. **The bound.** Domain: every `u32`, every `f32` bit pattern. Error: none; the result is the stable sort of the order stated.
7. **Checks.** `tests/radix.rs`: random keys up to 65 537 of them, many equal keys, sorted, reversed and extreme keys against a stable reference; skipped passes; refusals; `f32` with signed zeros, both NaNs, infinities and subnormals against `total_cmp`. The benchmark is `examples/radix_sort.rs`.
8. **The GPU.** No shader form yet.
9. **Runtime fallback.** Not applicable: no sign is decided.
10. **Tables.** None.

What it does not do: it is not adaptive to presorted input. A nearly sorted list takes the same passes as a random one; only a narrow range of keys skips passes. The measurement shows the standard library's adaptive sort faster on nearly sorted keys; a presorted path would be a method change with its own measurement.

## How to test

`cargo test -p calc-kernels`

`cargo run --release -p calc-kernels --example philox_fill` measures `philox4x32_10_fill` against a hand-written loop over the same slice and checks that both give the same words.

`cargo run --release -p calc-kernels --example radix_sort` measures `radix_sort_u32` against a hand-written loop with the same result on random, narrow and nearly sorted keys, with the standard library's stable sort for comparison.
