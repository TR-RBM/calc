use std::fmt::Write as _;

use calc_numbers::Integer;

const CHUNK_DIGITS: usize = 18;
const CHUNK_BASE: i64 = 1_000_000_000_000_000_000;
const MILLISECONDS_PER_SECOND: u64 = 1000;
const SECONDS_PER_DAY: u64 = 86_400;
const SECONDS_PER_HOUR: u64 = 3600;
const SECONDS_PER_MINUTE: u64 = 60;
const DAYS_PER_ERA: u64 = 146_097;
const YEARS_PER_ERA: u64 = 400;
const DAYS_FROM_CIVIL_ORIGIN_TO_UNIX_EPOCH: u64 = 719_468;
const TIMESTAMP_LENGTH: usize = 24;

pub(crate) fn integer_to_decimal(value: &Integer) -> String {
    if value.is_zero() {
        return "0".to_string();
    }
    let base = Integer::from(CHUNK_BASE);
    let mut rest = value.absolute();
    let mut chunks = Vec::new();
    while !rest.is_zero() {
        let Ok((quotient, remainder)) = rest.div_rem_euclid(&base) else {
            unreachable!()
        };
        chunks.push(remainder.to_i64().unwrap_or_default());
        rest = quotient;
    }
    let mut text = String::new();
    if value.is_negative() {
        text.push('-');
    }
    for (position, chunk) in chunks.iter().rev().enumerate() {
        if position == 0 {
            let _ = write!(text, "{chunk}");
        } else {
            let _ = write!(text, "{chunk:018}");
        }
    }
    text
}

pub(crate) fn integer_from_decimal(text: &str) -> Option<Integer> {
    let (is_negative, digits) = match text.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, text),
    };
    let is_canonical = !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && !(is_negative && digits == "0");
    if !is_canonical {
        return None;
    }
    let base = Integer::from(CHUNK_BASE);
    let first_length = match digits.len() % CHUNK_DIGITS {
        0 => CHUNK_DIGITS,
        partial => partial,
    };
    let mut magnitude = Integer::zero();
    let mut start = 0;
    let mut end = first_length;
    while start < digits.len() {
        let chunk: i64 = digits.get(start..end)?.parse().ok()?;
        magnitude = &(&magnitude * &base) + &Integer::from(chunk);
        start = end;
        end += CHUNK_DIGITS;
    }
    Some(if is_negative {
        magnitude.negated()
    } else {
        magnitude
    })
}

pub(crate) fn timestamp_to_text(milliseconds_since_unix_epoch: u64) -> String {
    let milliseconds = milliseconds_since_unix_epoch % MILLISECONDS_PER_SECOND;
    let seconds = milliseconds_since_unix_epoch / MILLISECONDS_PER_SECOND;
    let days = seconds / SECONDS_PER_DAY;
    let second_of_day = seconds % SECONDS_PER_DAY;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{milliseconds:03}Z",
        second_of_day / SECONDS_PER_HOUR,
        second_of_day % SECONDS_PER_HOUR / SECONDS_PER_MINUTE,
        second_of_day % SECONDS_PER_MINUTE,
    )
}

pub(crate) struct CivilTime {
    pub year: u64,
    pub month: u64,
    pub day: u64,
    pub hour: u64,
    pub minute: u64,
    pub second: u64,
}

pub(crate) fn civil_time(milliseconds_since_unix_epoch: u64) -> CivilTime {
    let seconds = milliseconds_since_unix_epoch / MILLISECONDS_PER_SECOND;
    let second_of_day = seconds % SECONDS_PER_DAY;
    let (year, month, day) = civil_from_days(seconds / SECONDS_PER_DAY);
    CivilTime {
        year,
        month,
        day,
        hour: second_of_day / SECONDS_PER_HOUR,
        minute: second_of_day % SECONDS_PER_HOUR / SECONDS_PER_MINUTE,
        second: second_of_day % SECONDS_PER_MINUTE,
    }
}

pub(crate) fn same_day(left: u64, right: u64) -> bool {
    let day = |milliseconds: u64| milliseconds / MILLISECONDS_PER_SECOND / SECONDS_PER_DAY;
    day(left) == day(right)
}

pub(crate) fn timestamp_from_text(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    if bytes.len() != TIMESTAMP_LENGTH
        || [
            (4, b'-'),
            (7, b'-'),
            (10, b'T'),
            (13, b':'),
            (16, b':'),
            (19, b'.'),
            (23, b'Z'),
        ]
        .iter()
        .any(|(position, separator)| bytes.get(*position) != Some(separator))
    {
        return None;
    }
    let field = |range: std::ops::Range<usize>| -> Option<u64> {
        let digits = text.get(range)?;
        digits
            .bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| digits.parse().ok())
            .flatten()
    };
    let year = field(0..4)?;
    let month = field(5..7)?;
    let day = field(8..10)?;
    let hour = field(11..13)?;
    let minute = field(14..16)?;
    let second = field(17..19)?;
    let millisecond = field(20..23)?;
    let days = days_from_civil(year, month, day)?;
    if hour >= 24 || minute >= 60 || second >= 60 {
        return None;
    }
    let seconds = days
        .checked_mul(SECONDS_PER_DAY)?
        .checked_add(hour * SECONDS_PER_HOUR + minute * SECONDS_PER_MINUTE + second)?;
    seconds
        .checked_mul(MILLISECONDS_PER_SECOND)?
        .checked_add(millisecond)
}

fn civil_from_days(days_since_unix_epoch: u64) -> (u64, u64, u64) {
    let days = days_since_unix_epoch + DAYS_FROM_CIVIL_ORIGIN_TO_UNIX_EPOCH;
    let era = days / DAYS_PER_ERA;
    let day_of_era = days - era * DAYS_PER_ERA;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * YEARS_PER_ERA + u64::from(month <= 2);
    (year, month, day)
}

fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let shifted_year = if month <= 2 {
        year.checked_sub(1)?
    } else {
        year
    };
    let era = shifted_year / YEARS_PER_ERA;
    let year_of_era = shifted_year - era * YEARS_PER_ERA;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * DAYS_PER_ERA + day_of_era).checked_sub(DAYS_FROM_CIVIL_ORIGIN_TO_UNIX_EPOCH)
}

fn days_in_month(year: u64, month: u64) -> u64 {
    let is_leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    match month {
        2 if is_leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_round_trips_through_decimal_text() {
        let values = [
            Integer::zero(),
            Integer::from(-7_i64),
            Integer::from(CHUNK_BASE),
            Integer::from(CHUNK_BASE - 1).negated(),
            Integer::from(10_u64).pow(40),
            &Integer::from(10_u64).pow(37) + &Integer::from(42_i64),
        ];

        let round_trips: Vec<Option<Integer>> = values
            .iter()
            .map(|value| integer_from_decimal(&integer_to_decimal(value)))
            .collect();

        assert_eq!(
            round_trips,
            values.iter().cloned().map(Some).collect::<Vec<_>>()
        );
    }

    #[test]
    fn decimal_text_pads_inner_chunks_with_zeros() {
        let value = &Integer::from(10_u64).pow(37) + &Integer::from(42_i64);

        assert_eq!(
            integer_to_decimal(&value),
            "10000000000000000000000000000000000042"
        );
    }

    #[test]
    fn non_canonical_integer_text_is_rejected() {
        let texts = ["", "-", "-0", "007", "1e3", "+1", " 1", "1.0"];

        let parsed: Vec<Option<Integer>> = texts
            .iter()
            .map(|text| integer_from_decimal(text))
            .collect();

        assert!(parsed.iter().all(Option::is_none));
    }

    #[test]
    fn unix_epoch_is_1970() {
        assert_eq!(timestamp_to_text(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn example_timestamp_of_the_session_format_round_trips() {
        let text = "2026-09-15T12:41:07.312Z";

        let milliseconds = timestamp_from_text(text).unwrap();

        assert_eq!(timestamp_to_text(milliseconds), text);
    }

    #[test]
    fn leap_day_is_converted() {
        assert_eq!(
            timestamp_from_text("2024-02-29T00:00:00.000Z"),
            Some(1_709_164_800_000)
        );
    }

    #[test]
    fn day_after_leap_day_in_non_leap_year_is_rejected() {
        assert_eq!(timestamp_from_text("2023-02-29T00:00:00.000Z"), None);
    }

    #[test]
    fn timestamp_before_epoch_is_rejected() {
        assert_eq!(timestamp_from_text("1969-12-31T23:59:59.999Z"), None);
    }

    #[test]
    fn timestamp_without_zone_letter_is_rejected() {
        assert_eq!(timestamp_from_text("2026-09-15T12:41:07.312+"), None);
    }

    #[test]
    fn every_day_of_four_centuries_round_trips() {
        let failures: Vec<u64> = (0..DAYS_PER_ERA)
            .map(|day| day * SECONDS_PER_DAY * MILLISECONDS_PER_SECOND)
            .filter(|milliseconds| {
                timestamp_from_text(&timestamp_to_text(*milliseconds)) != Some(*milliseconds)
            })
            .collect();

        assert!(failures.is_empty());
    }
}
