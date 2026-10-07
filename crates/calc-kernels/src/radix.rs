pub const RADIX_BUCKETS: usize = 256;

const DIGIT_BITS: u32 = 8;
const PASSES: u32 = 4;
const SIGN_BIT: u32 = 0x8000_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadixRefusal {
    LengthsDiffer,
    TooLong,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RadixPasses {
    pub made: u32,
    pub skipped: u32,
}

pub fn radix_sort_u32(
    keys: &mut [u32],
    payload: &mut [u32],
    scratch_keys: &mut [u32],
    scratch_payload: &mut [u32],
    histogram: &mut [u32; RADIX_BUCKETS],
) -> Result<RadixPasses, RadixRefusal> {
    let length = keys.len();
    refuse_unless_sortable(
        length,
        &[payload.len(), scratch_keys.len(), scratch_payload.len()],
    )?;
    let mut passes = RadixPasses {
        made: 0,
        skipped: 0,
    };
    let mut in_scratch = false;
    for pass in 0..PASSES {
        let shift = pass * DIGIT_BITS;
        let (from_keys, from_payload, to_keys, to_payload) = if in_scratch {
            (&*scratch_keys, &*scratch_payload, &mut *keys, &mut *payload)
        } else {
            (&*keys, &*payload, &mut *scratch_keys, &mut *scratch_payload)
        };
        histogram.fill(0);
        for key in from_keys {
            histogram[digit(*key, shift)] += 1;
        }
        if histogram
            .iter()
            .any(|count| usize::try_from(*count).is_ok_and(|count| count == length))
        {
            passes.skipped += 1;
            continue;
        }
        let mut start = 0;
        for count in histogram.iter_mut() {
            let size = *count;
            *count = start;
            start += size;
        }
        for (key, item) in from_keys.iter().zip(from_payload) {
            let bucket = &mut histogram[digit(*key, shift)];
            let place = usize::try_from(*bucket).unwrap_or(0);
            to_keys[place] = *key;
            to_payload[place] = *item;
            *bucket += 1;
        }
        in_scratch = !in_scratch;
        passes.made += 1;
    }
    if in_scratch {
        keys.copy_from_slice(scratch_keys);
        payload.copy_from_slice(scratch_payload);
    }
    Ok(passes)
}

pub fn radix_sort_f32(
    keys: &mut [f32],
    payload: &mut [u32],
    ordered: &mut [u32],
    scratch_keys: &mut [u32],
    scratch_payload: &mut [u32],
    histogram: &mut [u32; RADIX_BUCKETS],
) -> Result<RadixPasses, RadixRefusal> {
    refuse_unless_sortable(
        keys.len(),
        &[
            payload.len(),
            ordered.len(),
            scratch_keys.len(),
            scratch_payload.len(),
        ],
    )?;
    for (key, bits) in keys.iter().zip(ordered.iter_mut()) {
        *bits = total_order_bits(*key);
    }
    let passes = radix_sort_u32(ordered, payload, scratch_keys, scratch_payload, histogram)?;
    for (key, bits) in keys.iter_mut().zip(ordered.iter()) {
        *key = from_total_order_bits(*bits);
    }
    Ok(passes)
}

pub fn total_order_bits(value: f32) -> u32 {
    let bits = value.to_bits();
    if bits & SIGN_BIT == 0 {
        bits | SIGN_BIT
    } else {
        !bits
    }
}

pub fn from_total_order_bits(bits: u32) -> f32 {
    f32::from_bits(if bits & SIGN_BIT == 0 {
        !bits
    } else {
        bits & !SIGN_BIT
    })
}

fn refuse_unless_sortable(length: usize, others: &[usize]) -> Result<(), RadixRefusal> {
    if others.iter().any(|other| *other != length) {
        return Err(RadixRefusal::LengthsDiffer);
    }
    if u32::try_from(length).is_err() {
        return Err(RadixRefusal::TooLong);
    }
    Ok(())
}

fn digit(key: u32, shift: u32) -> usize {
    usize::try_from((key >> shift) & 0xFF).unwrap_or(0)
}
