use std::hint::black_box;
use std::time::Instant;

use calc_kernels::{philox4x32_10_at, philox4x32_10_fill};

const WORDS: usize = 1 << 20;
const REPEATS: u32 = 20;

fn hand_written(seed: u64, stream: u64, first_index: u64, words: &mut [u32]) {
    let mut index = first_index;
    let mut position = 0;
    while position < words.len() {
        let block = philox4x32_10_at(seed, stream, index);
        for word in block {
            if position < words.len() {
                words[position] = word;
                position += 1;
            }
        }
        index = index.wrapping_add(1);
    }
}

fn best_of(run: &mut dyn FnMut()) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..REPEATS {
        let started = Instant::now();
        run();
        best = best.min(started.elapsed().as_secs_f64());
    }
    best
}

fn main() {
    let mut kernel = vec![0_u32; WORDS];
    let mut reference = vec![0_u32; WORDS];
    let kernel_seconds = best_of(&mut || philox4x32_10_fill(black_box(1), 2, 3, &mut kernel));
    let reference_seconds = best_of(&mut || hand_written(black_box(1), 2, 3, &mut reference));
    assert_eq!(kernel, reference);
    let per_word = |seconds: f64| seconds * 1e9 / WORDS as f64;
    println!(
        "philox4x32_10_fill: {WORDS} words, best of {REPEATS}: {:.3} ms, {:.3} ns per word",
        kernel_seconds * 1e3,
        per_word(kernel_seconds)
    );
    println!(
        "hand-written loop:  {WORDS} words, best of {REPEATS}: {:.3} ms, {:.3} ns per word",
        reference_seconds * 1e3,
        per_word(reference_seconds)
    );
}
