//! UTC timestamps, formatted without a date-time dependency.
//!
//! The manifest is committed and read by people, so a timestamp in it is
//! RFC 3339 rather than a count of seconds. That is the only reason this module
//! exists: pulling in a calendar crate to format one field, in a crate whose
//! whole job is to own as few bytes as possible, is a dependency nobody asked
//! for.
//!
//! Nothing here parses, localises, or does arithmetic on dates. It converts an
//! instant to `YYYY-MM-DDTHH:MM:SSZ` and stops.

use std::time::{SystemTime, UNIX_EPOCH};

/// A source of the current time.
///
/// Injected rather than called, so a plan's output is a function of its inputs.
/// A test that could not fix the clock would have to assert around the one
/// field it cannot predict.
pub trait Clock {
    /// Seconds since the Unix epoch, UTC.
    fn now_unix(&self) -> i64;
}

/// The real clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            // A clock before 1970 is a broken machine, not a case to model.
            .unwrap_or(0)
    }
}

/// A clock frozen at a chosen instant, for tests and for reproducing a plan.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

/// Format seconds since the Unix epoch as RFC 3339 in UTC.
///
/// Proleptic Gregorian, which is what RFC 3339 specifies. Leap seconds are not
/// represented, because Unix time does not carry them.
pub fn rfc3339_utc(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let secs_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    )
}

/// Days since 1970-01-01 to a civil (year, month, day).
///
/// Howard Hinnant's `civil_from_days`, which shifts the era to start in March so
/// the leap day lands at the end of a year and the month-length table becomes a
/// single expression. Translated rather than invented: the naive version gets
/// the year-2000 style century rules subtly wrong.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_is_1970() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_instants_round_trip_against_hand_computed_values() {
        assert_eq!(rfc3339_utc(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(rfc3339_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn century_leap_rules_are_the_gregorian_ones() {
        // 1900 was NOT a leap year, 2000 was. A naive `% 4` gets 1900 wrong,
        // which is the whole reason this is Hinnant's algorithm.
        assert_eq!(rfc3339_utc(-2_203_891_200), "1900-03-01T00:00:00Z");
        assert_eq!(rfc3339_utc(951_868_800), "2000-03-01T00:00:00Z");
    }

    #[test]
    fn instants_before_the_epoch_do_not_wrap() {
        assert_eq!(rfc3339_utc(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn a_fixed_clock_is_fixed() {
        assert_eq!(FixedClock(42).now_unix(), 42);
    }
}
