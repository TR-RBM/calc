use calc_exec::ReduceOperation;

pub const SOFTFLOAT_WGSL: &str = r#"fn sf_is_nan(x: u32) -> bool {
    return (x & 0x7fffffffu) > 0x7f800000u;
}

fn sf_unpack_exponent(x: u32) -> i32 {
    let e = i32((x >> 23u) & 0xffu);
    return select(e, 1, e == 0);
}

fn sf_unpack_mantissa(x: u32) -> u32 {
    let f = x & 0x7fffffu;
    return select(f | 0x800000u, f, ((x >> 23u) & 0xffu) == 0u);
}

fn sf_encode(sign: u32, exponent: i32, mantissa: u32, guard_in: bool, sticky_in: bool) -> u32 {
    var e = exponent;
    var m = mantissa;
    var g = guard_in;
    var st = sticky_in;
    if (e <= 0) {
        let shift = 1 - e;
        if (shift > 25) {
            st = st || g || m != 0u;
            g = false;
            m = 0u;
        } else {
            let s = u32(shift);
            let new_guard = ((m >> (s - 1u)) & 1u) == 1u;
            var lower = 0u;
            if (s >= 2u) {
                lower = m & ((1u << (s - 1u)) - 1u);
            }
            st = st || g || lower != 0u;
            g = new_guard;
            m = m >> s;
        }
        e = 0;
    }
    if (g && (st || (m & 1u) == 1u)) {
        m = m + 1u;
        if (m == 0x1000000u) {
            m = m >> 1u;
            e = e + 1;
        }
    }
    if (e == 0) {
        return sign | m;
    }
    if (e >= 255) {
        return sign | 0x7f800000u;
    }
    return sign | (u32(e) << 23u) | (m & 0x7fffffu);
}

fn sf_mul(a: u32, b: u32) -> u32 {
    let sign = (a ^ b) & 0x80000000u;
    if (sf_is_nan(a) || sf_is_nan(b)) {
        return 0x7fc00000u;
    }
    let ma = a & 0x7fffffffu;
    let mb = b & 0x7fffffffu;
    if (ma == 0x7f800000u || mb == 0x7f800000u) {
        if (ma == 0u || mb == 0u) {
            return 0x7fc00000u;
        }
        return sign | 0x7f800000u;
    }
    if (ma == 0u || mb == 0u) {
        return sign;
    }
    var ea = sf_unpack_exponent(ma);
    var xa = sf_unpack_mantissa(ma);
    var eb = sf_unpack_exponent(mb);
    var xb = sf_unpack_mantissa(mb);
    let za = countLeadingZeros(xa) - 8u;
    xa = xa << za;
    ea = ea - i32(za);
    let zb = countLeadingZeros(xb) - 8u;
    xb = xb << zb;
    eb = eb - i32(zb);
    let ah = xa >> 12u;
    let al = xa & 0xfffu;
    let bh = xb >> 12u;
    let bl = xb & 0xfffu;
    let lo = al * bl;
    let mid = ah * bl + al * bh;
    let hi = ah * bh;
    let t0 = lo + ((mid & 0xfffu) << 12u);
    let low = t0 & 0xffffffu;
    let high = hi + (mid >> 12u) + (t0 >> 24u);
    var e = ea + eb - 127;
    if (high >= 0x800000u) {
        return sf_encode(sign, e + 1, high, ((low >> 23u) & 1u) == 1u, (low & 0x7fffffu) != 0u);
    }
    return sf_encode(sign, e, (high << 1u) | (low >> 23u), ((low >> 22u) & 1u) == 1u, (low & 0x3fffffu) != 0u);
}

fn sf_add(a: u32, b: u32) -> u32 {
    if (sf_is_nan(a) || sf_is_nan(b)) {
        return 0x7fc00000u;
    }
    let ma = a & 0x7fffffffu;
    let mb = b & 0x7fffffffu;
    if (ma == 0x7f800000u || mb == 0x7f800000u) {
        if (ma == 0x7f800000u && mb == 0x7f800000u && ((a ^ b) & 0x80000000u) != 0u) {
            return 0x7fc00000u;
        }
        return select(b, a, ma == 0x7f800000u);
    }
    if (ma == 0u && mb == 0u) {
        return a & b;
    }
    if (ma == 0u) {
        return b;
    }
    if (mb == 0u) {
        return a;
    }
    let a_is_big = ma >= mb;
    let big = select(b, a, a_is_big);
    let small = select(a, b, a_is_big);
    let sign = big & 0x80000000u;
    let eb = sf_unpack_exponent(big & 0x7fffffffu);
    let es = sf_unpack_exponent(small & 0x7fffffffu);
    var e = eb;
    let big_m = sf_unpack_mantissa(big & 0x7fffffffu) << 6u;
    var small_m = sf_unpack_mantissa(small & 0x7fffffffu) << 6u;
    let d = u32(eb - es);
    if (d >= 32u) {
        small_m = select(0u, 1u, small_m != 0u);
    } else if (d > 0u) {
        let sticky = (small_m & ((1u << d) - 1u)) != 0u;
        small_m = (small_m >> d) | select(0u, 1u, sticky);
    }
    var s = select(big_m - small_m, big_m + small_m, ((a ^ b) & 0x80000000u) == 0u);
    if (s == 0u) {
        return 0u;
    }
    if (s >= 0x40000000u) {
        s = (s >> 1u) | (s & 1u);
        e = e + 1;
    } else {
        let z = max(min(i32(countLeadingZeros(s)) - 2, e - 1), 0);
        s = s << u32(z);
        e = e - z;
    }
    var m = s >> 6u;
    let g = ((s >> 5u) & 1u) == 1u;
    let st = (s & 0x1fu) != 0u;
    if (m < 0x800000u) {
        if (g && (st || (m & 1u) == 1u)) {
            m = m + 1u;
        }
        return sign | m;
    }
    return sf_encode(sign, e, m, g, st);
}

fn sf_sub(a: u32, b: u32) -> u32 {
    return sf_add(a, b ^ 0x80000000u);
}

fn sf_greater(a: u32, b: u32) -> bool {
    if (sf_is_nan(a) || sf_is_nan(b)) {
        return false;
    }
    let ma = a & 0x7fffffffu;
    let mb = b & 0x7fffffffu;
    if (ma == 0u && mb == 0u) {
        return false;
    }
    let sa = (a & 0x80000000u) != 0u;
    let sb = (b & 0x80000000u) != 0u;
    if (sa != sb) {
        return !sa;
    }
    return select(ma > mb, ma < mb, sa);
}

fn sf_maximum(a: u32, b: u32) -> u32 {
    if (sf_is_nan(a)) {
        return a | 0x00400000u;
    }
    if (sf_is_nan(b)) {
        return b | 0x00400000u;
    }
    let equal = a == b || ((a | b) & 0x7fffffffu) == 0u;
    return select(b, a, sf_greater(a, b) || (equal && (a & 0x80000000u) == 0u));
}

fn sf_minimum(a: u32, b: u32) -> u32 {
    if (sf_is_nan(a)) {
        return a | 0x00400000u;
    }
    if (sf_is_nan(b)) {
        return b | 0x00400000u;
    }
    let equal = a == b || ((a | b) & 0x7fffffffu) == 0u;
    return select(b, a, sf_greater(b, a) || (equal && (a & 0x80000000u) != 0u));
}
fn sf_trunc(x: u32) -> u32 {
    if (sf_is_nan(x)) {
        return x | 0x00400000u;
    }
    let exponent = i32((x >> 23u) & 0xffu) - 127;
    if (exponent >= 23) {
        return x;
    }
    if (exponent < 0) {
        return x & 0x80000000u;
    }
    let mask = (1u << u32(23 - exponent)) - 1u;
    return x & ~mask;
}

fn sf_floor(x: u32) -> u32 {
    if (sf_is_nan(x)) {
        return x | 0x00400000u;
    }
    let exponent = i32((x >> 23u) & 0xffu) - 127;
    if (exponent >= 23) {
        return x;
    }
    let negative = (x & 0x80000000u) != 0u;
    if (exponent < 0) {
        if (negative && (x & 0x7fffffffu) != 0u) {
            return 0xbf800000u;
        }
        return x & 0x80000000u;
    }
    let mask = (1u << u32(23 - exponent)) - 1u;
    let truncated = x & ~mask;
    if (negative && (x & mask) != 0u) {
        return truncated + mask + 1u;
    }
    return truncated;
}

fn sf_ceil(x: u32) -> u32 {
    if (sf_is_nan(x)) {
        return x | 0x00400000u;
    }
    let exponent = i32((x >> 23u) & 0xffu) - 127;
    if (exponent >= 23) {
        return x;
    }
    let negative = (x & 0x80000000u) != 0u;
    if (exponent < 0) {
        if (!negative && (x & 0x7fffffffu) != 0u) {
            return 0x3f800000u;
        }
        return x & 0x80000000u;
    }
    let mask = (1u << u32(23 - exponent)) - 1u;
    let truncated = x & ~mask;
    if (!negative && (x & mask) != 0u) {
        return truncated + mask + 1u;
    }
    return truncated;
}

fn sf_round_ties_even(x: u32) -> u32 {
    if (sf_is_nan(x)) {
        return x | 0x00400000u;
    }
    let exponent = i32((x >> 23u) & 0xffu) - 127;
    if (exponent >= 23) {
        return x;
    }
    let sign = x & 0x80000000u;
    if (exponent < -1) {
        return sign;
    }
    if (exponent == -1) {
        if ((x & 0x7fffffffu) == 0x3f000000u) {
            return sign;
        }
        return sign | 0x3f800000u;
    }
    let mask = (1u << u32(23 - exponent)) - 1u;
    let step = mask + 1u;
    let truncated = x & ~mask;
    let fraction = x & mask;
    let half = step >> 1u;
    if (fraction > half || (fraction == half && (truncated & step) != 0u)) {
        return truncated + step;
    }
    return truncated;
}
fn sf_div(a: u32, b: u32) -> u32 {
    let sign = (a ^ b) & 0x80000000u;
    if (sf_is_nan(a) || sf_is_nan(b)) {
        return 0x7fc00000u;
    }
    let ma = a & 0x7fffffffu;
    let mb = b & 0x7fffffffu;
    if (ma == 0x7f800000u) {
        if (mb == 0x7f800000u) {
            return 0x7fc00000u;
        }
        return sign | 0x7f800000u;
    }
    if (mb == 0x7f800000u) {
        return sign;
    }
    if (mb == 0u) {
        if (ma == 0u) {
            return 0x7fc00000u;
        }
        return sign | 0x7f800000u;
    }
    if (ma == 0u) {
        return sign;
    }
    var ea = sf_unpack_exponent(ma);
    var xa = sf_unpack_mantissa(ma);
    var eb = sf_unpack_exponent(mb);
    var xb = sf_unpack_mantissa(mb);
    let za = countLeadingZeros(xa) - 8u;
    xa = xa << za;
    ea = ea - i32(za);
    let zb = countLeadingZeros(xb) - 8u;
    xb = xb << zb;
    eb = eb - i32(zb);
    var exponent = ea - eb + 127;
    var remainder = xa;
    if (remainder < xb) {
        remainder = remainder << 1u;
        exponent = exponent - 1;
    }
    var quotient = 0u;
    for (var step = 0u; step < 25u; step = step + 1u) {
        quotient = quotient << 1u;
        if (remainder >= xb) {
            remainder = remainder - xb;
            quotient = quotient | 1u;
        }
        remainder = remainder << 1u;
    }
    return sf_encode(sign, exponent, quotient >> 1u, (quotient & 1u) == 1u, remainder != 0u);
}

fn sf_sqrt(a: u32) -> u32 {
    if (sf_is_nan(a)) {
        return 0x7fc00000u;
    }
    let magnitude = a & 0x7fffffffu;
    if (magnitude == 0u) {
        return a;
    }
    if ((a & 0x80000000u) != 0u) {
        return 0x7fc00000u;
    }
    if (magnitude == 0x7f800000u) {
        return a;
    }
    var exponent = sf_unpack_exponent(magnitude);
    var mantissa = sf_unpack_mantissa(magnitude);
    let zeros = countLeadingZeros(mantissa) - 8u;
    mantissa = mantissa << zeros;
    exponent = exponent - i32(zeros);
    var unbiased = exponent - 127;
    if ((unbiased % 2) != 0) {
        mantissa = mantissa << 1u;
        unbiased = unbiased - 1;
    }
    var remainder = 0u;
    var root = 0u;
    for (var step = 0; step < 25; step = step + 1) {
        let place = 48 - 2 * step;
        var digit = 0u;
        if (place >= 25) {
            digit = (mantissa >> u32(place - 25)) & 3u;
        } else if (place == 24) {
            digit = (mantissa & 1u) << 1u;
        }
        remainder = (remainder << 2u) | digit;
        root = root << 1u;
        let trial = (root << 1u) | 1u;
        if (remainder >= trial) {
            remainder = remainder - trial;
            root = root | 1u;
        }
    }
    return sf_encode(0u, unbiased / 2 + 127, root >> 1u, (root & 1u) == 1u, remainder != 0u);
}
"#;

const SIGN: u32 = 0x8000_0000;
const MAGNITUDE: u32 = 0x7fff_ffff;
const QUIET_NAN: u32 = 0x7fc0_0000;
const INFINITY: u32 = 0x7f80_0000;
const ONE: u32 = 0x3f80_0000;
const NEGATIVE_ONE: u32 = 0xbf80_0000;
const HALF: u32 = 0x3f00_0000;

fn signed(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn unsigned(value: i32) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

fn exponent_of(x: u32) -> i32 {
    signed((x >> 23) & 0xff) - 127
}

fn mask_below(exponent: i32) -> u32 {
    (1u32 << unsigned(23 - exponent)) - 1
}

pub fn is_nan(x: u32) -> bool {
    (x & MAGNITUDE) > INFINITY
}

fn unpack_exponent(x: u32) -> i32 {
    let exponent = signed((x >> 23) & 0xff);
    if exponent == 0 { 1 } else { exponent }
}

fn unpack_mantissa(x: u32) -> u32 {
    let fraction = x & 0x7f_ffff;
    if (x >> 23) & 0xff == 0 {
        fraction
    } else {
        fraction | 0x80_0000
    }
}

fn encode(sign: u32, exponent: i32, mantissa: u32, guard: bool, sticky: bool) -> u32 {
    let mut e = exponent;
    let mut m = mantissa;
    let mut g = guard;
    let mut st = sticky;
    if e <= 0 {
        let shift = 1 - e;
        if shift > 25 {
            st = st || g || m != 0;
            g = false;
            m = 0;
        } else {
            let s = unsigned(shift);
            let new_guard = (m >> (s - 1)) & 1 == 1;
            let lower = if s >= 2 { m & ((1 << (s - 1)) - 1) } else { 0 };
            st = st || g || lower != 0;
            g = new_guard;
            m >>= s;
        }
        e = 0;
    }
    if g && (st || (m & 1) == 1) {
        m += 1;
        if m == 1 << 24 {
            m >>= 1;
            e += 1;
        }
    }
    if e == 0 {
        return sign | m;
    }
    if e >= 255 {
        return sign | INFINITY;
    }
    sign | (unsigned(e) << 23) | (m & 0x7f_ffff)
}

pub fn multiply(a: u32, b: u32) -> u32 {
    let sign = (a ^ b) & SIGN;
    if is_nan(a) || is_nan(b) {
        return QUIET_NAN;
    }
    let (magnitude_a, magnitude_b) = (a & MAGNITUDE, b & MAGNITUDE);
    if magnitude_a == INFINITY || magnitude_b == INFINITY {
        if magnitude_a == 0 || magnitude_b == 0 {
            return QUIET_NAN;
        }
        return sign | INFINITY;
    }
    if magnitude_a == 0 || magnitude_b == 0 {
        return sign;
    }
    let mut exponent_a = unpack_exponent(magnitude_a);
    let mut mantissa_a = unpack_mantissa(magnitude_a);
    let mut exponent_b = unpack_exponent(magnitude_b);
    let mut mantissa_b = unpack_mantissa(magnitude_b);
    let shift_a = mantissa_a.leading_zeros() - 8;
    mantissa_a <<= shift_a;
    exponent_a -= signed(shift_a);
    let shift_b = mantissa_b.leading_zeros() - 8;
    mantissa_b <<= shift_b;
    exponent_b -= signed(shift_b);
    let (high_a, low_a) = (mantissa_a >> 12, mantissa_a & 0xfff);
    let (high_b, low_b) = (mantissa_b >> 12, mantissa_b & 0xfff);
    let low = low_a * low_b;
    let middle = high_a * low_b + low_a * high_b;
    let high = high_a * high_b;
    let joined = low + ((middle & 0xfff) << 12);
    let product_low = joined & 0xff_ffff;
    let product_high = high + (middle >> 12) + (joined >> 24);
    let exponent = exponent_a + exponent_b - 127;
    if product_high >= 1 << 23 {
        return encode(
            sign,
            exponent + 1,
            product_high,
            (product_low >> 23) & 1 == 1,
            (product_low & 0x7f_ffff) != 0,
        );
    }
    encode(
        sign,
        exponent,
        (product_high << 1) | (product_low >> 23),
        (product_low >> 22) & 1 == 1,
        (product_low & 0x3f_ffff) != 0,
    )
}

pub fn add(a: u32, b: u32) -> u32 {
    if is_nan(a) || is_nan(b) {
        return QUIET_NAN;
    }
    let (magnitude_a, magnitude_b) = (a & MAGNITUDE, b & MAGNITUDE);
    if magnitude_a == INFINITY || magnitude_b == INFINITY {
        if magnitude_a == INFINITY && magnitude_b == INFINITY && (a ^ b) & SIGN != 0 {
            return QUIET_NAN;
        }
        return if magnitude_a == INFINITY { a } else { b };
    }
    if magnitude_a == 0 && magnitude_b == 0 {
        return a & b;
    }
    if magnitude_a == 0 {
        return b;
    }
    if magnitude_b == 0 {
        return a;
    }
    let (big, small) = if magnitude_b > magnitude_a {
        (b, a)
    } else {
        (a, b)
    };
    let sign = big & SIGN;
    let exponent_big = unpack_exponent(big & MAGNITUDE);
    let exponent_small = unpack_exponent(small & MAGNITUDE);
    let mut exponent = exponent_big;
    let mantissa_big = unpack_mantissa(big & MAGNITUDE) << 6;
    let mut mantissa_small = unpack_mantissa(small & MAGNITUDE) << 6;
    let distance = unsigned(exponent_big - exponent_small);
    if distance >= 32 {
        mantissa_small = u32::from(mantissa_small != 0);
    } else if distance > 0 {
        let sticky = (mantissa_small & ((1 << distance) - 1)) != 0;
        mantissa_small = (mantissa_small >> distance) | u32::from(sticky);
    }
    let mut sum = if (a ^ b) & SIGN == 0 {
        mantissa_big + mantissa_small
    } else {
        mantissa_big - mantissa_small
    };
    if sum == 0 {
        return 0;
    }
    if sum >= 1 << 30 {
        sum = (sum >> 1) | (sum & 1);
        exponent += 1;
    } else {
        let shift = (signed(sum.leading_zeros()) - 2).min(exponent - 1).max(0);
        sum <<= unsigned(shift);
        exponent -= shift;
    }
    let mut mantissa = sum >> 6;
    let guard = (sum >> 5) & 1 == 1;
    let sticky = (sum & 0x1f) != 0;
    if mantissa < 1 << 23 {
        if guard && (sticky || mantissa & 1 == 1) {
            mantissa += 1;
        }
        return sign | mantissa;
    }
    encode(sign, exponent, mantissa, guard, sticky)
}

pub fn subtract(a: u32, b: u32) -> u32 {
    add(a, b ^ SIGN)
}

pub fn greater(a: u32, b: u32) -> bool {
    if is_nan(a) || is_nan(b) {
        return false;
    }
    let (magnitude_a, magnitude_b) = (a & MAGNITUDE, b & MAGNITUDE);
    if magnitude_a == 0 && magnitude_b == 0 {
        return false;
    }
    let negative_a = a & SIGN != 0;
    if negative_a != (b & SIGN != 0) {
        return !negative_a;
    }
    if negative_a {
        magnitude_a < magnitude_b
    } else {
        magnitude_a > magnitude_b
    }
}

fn equal(a: u32, b: u32) -> bool {
    a == b || ((a | b) & MAGNITUDE) == 0
}

pub fn compares_equal(a: u32, b: u32) -> bool {
    !is_nan(a) && !is_nan(b) && equal(a, b)
}

pub fn less_or_equal(a: u32, b: u32) -> bool {
    !is_nan(a) && !is_nan(b) && !greater(a, b)
}

pub fn maximum(a: u32, b: u32) -> u32 {
    if is_nan(a) {
        return a | 0x0040_0000;
    }
    if is_nan(b) {
        return b | 0x0040_0000;
    }
    if greater(a, b) || (equal(a, b) && a & SIGN == 0) {
        a
    } else {
        b
    }
}

pub fn minimum(a: u32, b: u32) -> u32 {
    if is_nan(a) {
        return a | 0x0040_0000;
    }
    if is_nan(b) {
        return b | 0x0040_0000;
    }
    if greater(b, a) || (equal(a, b) && a & SIGN != 0) {
        a
    } else {
        b
    }
}

pub fn truncate(x: u32) -> u32 {
    if is_nan(x) {
        return x | 0x0040_0000;
    }
    let exponent = exponent_of(x);
    if exponent >= 23 {
        return x;
    }
    if exponent < 0 {
        return x & SIGN;
    }
    let mask = mask_below(exponent);
    x & !mask
}

pub fn floor(x: u32) -> u32 {
    if is_nan(x) {
        return x | 0x0040_0000;
    }
    let exponent = exponent_of(x);
    if exponent >= 23 {
        return x;
    }
    let negative = x & SIGN != 0;
    if exponent < 0 {
        if negative && x & MAGNITUDE != 0 {
            return NEGATIVE_ONE;
        }
        return x & SIGN;
    }
    let mask = mask_below(exponent);
    let truncated = x & !mask;
    if negative && x & mask != 0 {
        return truncated + mask + 1;
    }
    truncated
}

pub fn ceil(x: u32) -> u32 {
    if is_nan(x) {
        return x | 0x0040_0000;
    }
    let exponent = exponent_of(x);
    if exponent >= 23 {
        return x;
    }
    let negative = x & SIGN != 0;
    if exponent < 0 {
        if !negative && x & MAGNITUDE != 0 {
            return ONE;
        }
        return x & SIGN;
    }
    let mask = mask_below(exponent);
    let truncated = x & !mask;
    if !negative && x & mask != 0 {
        return truncated + mask + 1;
    }
    truncated
}

pub fn round_ties_even(x: u32) -> u32 {
    if is_nan(x) {
        return x | 0x0040_0000;
    }
    let exponent = exponent_of(x);
    if exponent >= 23 {
        return x;
    }
    let sign = x & SIGN;
    if exponent < -1 {
        return sign;
    }
    if exponent == -1 {
        if x & MAGNITUDE == HALF {
            return sign;
        }
        return sign | ONE;
    }
    let mask = mask_below(exponent);
    let step = mask + 1;
    let truncated = x & !mask;
    let fraction = x & mask;
    let half = step >> 1;
    if fraction > half || (fraction == half && truncated & step != 0) {
        return truncated + step;
    }
    truncated
}

pub fn divide(a: u32, b: u32) -> u32 {
    let sign = (a ^ b) & SIGN;
    if is_nan(a) || is_nan(b) {
        return QUIET_NAN;
    }
    let (magnitude_a, magnitude_b) = (a & MAGNITUDE, b & MAGNITUDE);
    if magnitude_a == INFINITY {
        if magnitude_b == INFINITY {
            return QUIET_NAN;
        }
        return sign | INFINITY;
    }
    if magnitude_b == INFINITY {
        return sign;
    }
    if magnitude_b == 0 {
        if magnitude_a == 0 {
            return QUIET_NAN;
        }
        return sign | INFINITY;
    }
    if magnitude_a == 0 {
        return sign;
    }
    let mut exponent_a = unpack_exponent(magnitude_a);
    let mut mantissa_a = unpack_mantissa(magnitude_a);
    let mut exponent_b = unpack_exponent(magnitude_b);
    let mut mantissa_b = unpack_mantissa(magnitude_b);
    let shift_a = mantissa_a.leading_zeros() - 8;
    mantissa_a <<= shift_a;
    exponent_a -= signed(shift_a);
    let shift_b = mantissa_b.leading_zeros() - 8;
    mantissa_b <<= shift_b;
    exponent_b -= signed(shift_b);
    let mut exponent = exponent_a - exponent_b + 127;
    let mut remainder = mantissa_a;
    if remainder < mantissa_b {
        remainder <<= 1;
        exponent -= 1;
    }
    let mut quotient = 0;
    for _ in 0..25 {
        quotient <<= 1;
        if remainder >= mantissa_b {
            remainder -= mantissa_b;
            quotient |= 1;
        }
        remainder <<= 1;
    }
    encode(
        sign,
        exponent,
        quotient >> 1,
        quotient & 1 == 1,
        remainder != 0,
    )
}

pub fn square_root(a: u32) -> u32 {
    if is_nan(a) {
        return QUIET_NAN;
    }
    let magnitude = a & MAGNITUDE;
    if magnitude == 0 {
        return a;
    }
    if a & SIGN != 0 {
        return QUIET_NAN;
    }
    if magnitude == INFINITY {
        return a;
    }
    let mut exponent = unpack_exponent(magnitude);
    let mut mantissa = unpack_mantissa(magnitude);
    let shift = mantissa.leading_zeros() - 8;
    mantissa <<= shift;
    exponent -= signed(shift);
    let mut unbiased = exponent - 127;
    if unbiased % 2 != 0 {
        mantissa <<= 1;
        unbiased -= 1;
    }
    let mut remainder = 0;
    let mut root = 0;
    for step in 0..25 {
        let place = 48 - 2 * step;
        let digit = if place >= 25 {
            (mantissa >> (place - 25)) & 3
        } else if place == 24 {
            (mantissa & 1) << 1
        } else {
            0
        };
        remainder = (remainder << 2) | digit;
        root <<= 1;
        let trial = (root << 1) | 1;
        if remainder >= trial {
            remainder -= trial;
            root |= 1;
        }
    }
    encode(
        0,
        unbiased / 2 + 127,
        root >> 1,
        root & 1 == 1,
        remainder != 0,
    )
}

pub fn combine(operation: ReduceOperation, a: u32, b: u32) -> u32 {
    match operation {
        ReduceOperation::Add => add(a, b),
        ReduceOperation::Mul => multiply(a, b),
        ReduceOperation::Min => minimum(a, b),
        ReduceOperation::Max => maximum(a, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_match(left: u32, right: u32) -> bool {
        left == right || (is_nan(left) && is_nan(right))
    }

    fn values() -> Vec<u32> {
        let mut found: Vec<u32> = calc_conformance::special_values_f32()
            .into_iter()
            .map(f32::to_bits)
            .collect();
        for exponent in -30..=30 {
            let power = (2.0_f32).powi(exponent);
            found.push(power.to_bits());
            found.push((-power).to_bits());
            found.push(power.next_up().to_bits());
            found.push(power.next_down().to_bits());
        }
        let mut state: u32 = 0x1234_5678;
        for _ in 0..600 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            found.push(state);
        }
        found
    }

    fn check(name: &str, mirror: fn(u32, u32) -> u32, native: fn(f32, f32) -> f32) {
        let values = values();
        for &a in &values {
            for &b in &values {
                let found = mirror(a, b);
                let wanted = native(f32::from_bits(a), f32::from_bits(b)).to_bits();
                assert!(
                    bits_match(found, wanted),
                    "{name}({a:#010x}, {b:#010x}) gave {found:#010x}, native f32 gives {wanted:#010x}"
                );
            }
        }
    }

    #[test]
    fn add_matches_native_f32() {
        check("add", add, |a, b| a + b);
    }

    #[test]
    fn subtract_matches_native_f32() {
        check("subtract", subtract, |a, b| a - b);
    }

    #[test]
    fn multiply_matches_native_f32() {
        check("multiply", multiply, |a, b| a * b);
    }

    #[test]
    fn divide_matches_native_f32() {
        check("divide", divide, |a, b| a / b);
    }

    #[test]
    fn square_root_matches_native_f32() {
        for &x in &values() {
            let found = square_root(x);
            let wanted = f32::from_bits(x).sqrt().to_bits();
            assert!(
                bits_match(found, wanted),
                "square_root({x:#010x}) gave {found:#010x}, native f32 gives {wanted:#010x}"
            );
        }
    }

    #[test]
    fn division_by_zero_and_of_zero_follow_the_signs() {
        let one = 1.0_f32.to_bits();
        let zero = 0.0_f32.to_bits();

        assert_eq!(divide(one, zero), f32::INFINITY.to_bits());
        assert_eq!(divide(one, zero | SIGN), f32::NEG_INFINITY.to_bits());
        assert_eq!(divide(zero, one), zero);
        assert_eq!(divide(zero | SIGN, one), zero | SIGN);
        assert!(is_nan(divide(zero, zero)));
    }

    #[test]
    fn square_root_of_a_negative_zero_is_a_negative_zero() {
        let found = square_root((-0.0_f32).to_bits());

        assert_eq!(found, (-0.0_f32).to_bits());
    }

    #[test]
    fn maximum_and_minimum_match_the_cpu_backend() {
        for &a in &values() {
            for &b in &values() {
                let (left, right) = (f32::from_bits(a), f32::from_bits(b));
                assert!(
                    bits_match(
                        maximum(a, b),
                        calc_numbers::maximum_f32(left, right).to_bits()
                    ),
                    "maximum({a:#010x}, {b:#010x})"
                );
                assert!(
                    bits_match(
                        minimum(a, b),
                        calc_numbers::minimum_f32(left, right).to_bits()
                    ),
                    "minimum({a:#010x}, {b:#010x})"
                );
            }
        }
    }

    #[test]
    fn comparisons_match_native_f32() {
        for &a in &values() {
            for &b in &values() {
                let (left, right) = (f32::from_bits(a), f32::from_bits(b));
                assert_eq!(greater(a, b), left > right, "greater({a:#010x}, {b:#010x})");
                assert_eq!(
                    less_or_equal(a, b),
                    left <= right,
                    "less_or_equal({a:#010x}, {b:#010x})"
                );
                assert_eq!(
                    compares_equal(a, b),
                    left == right,
                    "compares_equal({a:#010x}, {b:#010x})"
                );
            }
        }
    }

    #[test]
    fn rounding_to_an_integer_matches_native_f32() {
        let mut values = values();
        for exponent in -2..=25 {
            for step in [-2, -1, 0, 1, 2] {
                let value = (2.0_f32).powi(exponent) + (step as f32) * 0.25;
                values.push(value.to_bits());
                values.push((-value).to_bits());
            }
        }
        for &x in &values {
            let value = f32::from_bits(x);
            for (name, found, wanted) in [
                ("floor", floor(x), value.floor()),
                ("ceil", ceil(x), value.ceil()),
                ("truncate", truncate(x), value.trunc()),
                (
                    "round_ties_even",
                    round_ties_even(x),
                    value.round_ties_even(),
                ),
            ] {
                assert!(
                    bits_match(found, wanted.to_bits()),
                    "{name}({x:#010x}) gave {found:#010x}, native f32 gives {:#010x}",
                    wanted.to_bits()
                );
            }
        }
    }

    #[test]
    fn rounding_quiets_a_signalling_nan_as_the_arithmetic_does() {
        let signalling = 0x7f80_0001;

        for found in [
            floor(signalling),
            ceil(signalling),
            truncate(signalling),
            round_ties_even(signalling),
        ] {
            assert_eq!(found, 0x7fc0_0001);
        }
    }

    #[test]
    fn rounding_a_subnormal_keeps_its_sign_and_reaches_one() {
        let smallest = f32::from_bits(1);

        assert_eq!(ceil(smallest.to_bits()), 1.0_f32.to_bits());
        assert_eq!(floor((-smallest).to_bits()), (-1.0_f32).to_bits());
        assert_eq!(floor(smallest.to_bits()), 0.0_f32.to_bits());
        assert_eq!(truncate((-smallest).to_bits()), (-0.0_f32).to_bits());
    }

    #[test]
    fn the_shader_text_declares_every_mirrored_function() {
        for wanted in [
            "fn sf_add(",
            "fn sf_sub(",
            "fn sf_mul(",
            "fn sf_maximum(",
            "fn sf_minimum(",
            "fn sf_greater(",
            "fn sf_is_nan(",
            "fn sf_floor(",
            "fn sf_ceil(",
            "fn sf_trunc(",
            "fn sf_round_ties_even(",
            "fn sf_div(",
            "fn sf_sqrt(",
        ] {
            assert!(SOFTFLOAT_WGSL.contains(wanted), "{wanted} is missing");
        }
    }
}
