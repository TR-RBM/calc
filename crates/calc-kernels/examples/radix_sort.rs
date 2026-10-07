use std::hint::black_box;
use std::time::Instant;

use calc_kernels::{RADIX_BUCKETS, philox4x32_10_fill, radix_sort_u32};

const LENGTH: usize = 1 << 20;
const REPEATS: u32 = 10;

fn hand_written(keys: &mut [u32], payload: &mut [u32]) {
    let mut scratch_keys = vec![0_u32; keys.len()];
    let mut scratch_payload = vec![0_u32; keys.len()];
    for shift in [0, 8, 16, 24] {
        let mut counts = [0_usize; 256];
        for key in keys.iter() {
            counts[((key >> shift) & 0xFF) as usize] += 1;
        }
        let mut start = 0;
        for count in counts.iter_mut() {
            let size = *count;
            *count = start;
            start += size;
        }
        for index in 0..keys.len() {
            let bucket = &mut counts[((keys[index] >> shift) & 0xFF) as usize];
            scratch_keys[*bucket] = keys[index];
            scratch_payload[*bucket] = payload[index];
            *bucket += 1;
        }
        keys.copy_from_slice(&scratch_keys);
        payload.copy_from_slice(&scratch_payload);
    }
}

fn best_of(run: &mut dyn FnMut() -> f64) -> f64 {
    (0..REPEATS).map(|_| run()).fold(f64::INFINITY, f64::min)
}

fn measure(name: &str, source: &[u32]) {
    let identity: Vec<u32> = (0..u32::try_from(source.len()).unwrap()).collect();
    let mut kernel_result = Vec::new();
    let kernel = best_of(&mut || {
        let (mut keys, mut payload) = (source.to_vec(), identity.clone());
        let (mut scratch_keys, mut scratch_payload) = (vec![0; keys.len()], vec![0; keys.len()]);
        let mut histogram = [0; RADIX_BUCKETS];
        let started = Instant::now();
        radix_sort_u32(
            black_box(&mut keys),
            &mut payload,
            &mut scratch_keys,
            &mut scratch_payload,
            &mut histogram,
        )
        .unwrap();
        let seconds = started.elapsed().as_secs_f64();
        kernel_result = payload;
        seconds
    });
    let mut loop_result = Vec::new();
    let reference = best_of(&mut || {
        let (mut keys, mut payload) = (source.to_vec(), identity.clone());
        let started = Instant::now();
        hand_written(black_box(&mut keys), &mut payload);
        let seconds = started.elapsed().as_secs_f64();
        loop_result = payload;
        seconds
    });
    let library = best_of(&mut || {
        let mut pairs: Vec<(u32, u32)> = source
            .iter()
            .copied()
            .zip(identity.iter().copied())
            .collect();
        let started = Instant::now();
        black_box(&mut pairs).sort_by_key(|(key, _)| *key);
        started.elapsed().as_secs_f64()
    });
    assert_eq!(kernel_result, loop_result);
    let per_key = |seconds: f64| seconds * 1e9 / source.len() as f64;
    println!(
        "{name}: {} keys, best of {REPEATS}: radix_sort_u32 {:.2} ns per key, hand-written loop {:.2} ns per key, std stable sort {:.2} ns per key",
        source.len(),
        per_key(kernel),
        per_key(reference),
        per_key(library)
    );
}

fn main() {
    let mut random = vec![0_u32; LENGTH];
    philox4x32_10_fill(1, 0, 0, &mut random);
    let narrow: Vec<u32> = random.iter().map(|key| key & 0xFFFF).collect();
    let mut nearly_sorted: Vec<u32> = (0..u32::try_from(LENGTH).unwrap()).collect();
    for pair in nearly_sorted.chunks_mut(64) {
        pair.swap(0, pair.len() - 1);
    }
    measure("random 32-bit keys", &random);
    measure("keys below 2^16", &narrow);
    measure("nearly sorted keys below 2^20", &nearly_sorted);
}
