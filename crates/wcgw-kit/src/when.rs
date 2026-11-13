//! Saying when, the same way in every app: how long ago, in a feed's column (`ago`); when, in a list of mail or chat
//! (`short`); and in full (`long`, `day`). Days come before months, as `5 Nov`; times are 24-hour.

use jiff::{Timestamp, Zoned};

/// How long before `now` `then` was, in three cells at most, as a feed's column says it: `now`, `5m`, `3h`, `2d`,
/// `4w`, `1y`. What's ahead of `now` (a clock ahead of ours) is `now`.
#[must_use]
pub fn ago(then: Timestamp, now: Timestamp) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const WEEK: i64 = 7 * DAY;
    const YEAR: i64 = 365 * DAY;
    let seconds = now.as_second().saturating_sub(then.as_second());
    match seconds {
        ..MINUTE => "now".to_owned(),
        MINUTE..HOUR => format!("{}m", seconds / MINUTE),
        HOUR..DAY => format!("{}h", seconds / HOUR),
        DAY..WEEK => format!("{}d", seconds / DAY),
        WEEK..YEAR => format!("{}w", seconds / WEEK),
        _ => format!("{}y", seconds / YEAR),
    }
}

/// When `then` was, in eight cells at most, as a list of mail or chat says it: the time on `now`'s day (`14:05`),
/// the day in `now`'s year (`5 Nov`), else the month (`Nov 2025`).
#[must_use]
pub fn short(then: &Zoned, now: &Zoned) -> String {
    let format = if then.date() == now.date() {
        "%H:%M"
    } else if then.year() == now.year() {
        "%-d %b"
    } else {
        "%b %Y"
    };
    then.strftime(format).to_string()
}

/// `then` in full: `Thu 5 Nov 2026, 14:05`.
#[must_use]
pub fn long(then: &Zoned) -> String {
    then.strftime("%a %-d %b %Y, %H:%M").to_string()
}

/// `then`'s day in full: `Thu 5 Nov 2026`.
#[must_use]
pub fn day(then: &Zoned) -> String {
    then.strftime("%a %-d %b %Y").to_string()
}

/// `then`'s time of day: `14:05`.
#[must_use]
pub fn clock(then: &Zoned) -> String {
    then.strftime("%H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use jiff::tz::TimeZone;

    use super::*;

    fn at(year: i16, month: i8, day: i8, hour: i8, minute: i8) -> Zoned {
        date(year, month, day)
            .at(hour, minute, 0, 0)
            .to_zoned(TimeZone::UTC)
            .expect("a valid time")
    }

    #[test]
    fn ago_is_three_cells_at_most() {
        let now = at(2026, 11, 13, 12, 0).timestamp();
        let before = |seconds: i64| now - jiff::SignedDuration::from_secs(seconds);

        assert_eq!(ago(before(59), now), "now");
        assert_eq!(ago(now + jiff::SignedDuration::from_secs(90), now), "now");
        assert_eq!(ago(before(5 * 60), now), "5m");
        assert_eq!(ago(before(3 * 3600), now), "3h");
        assert_eq!(ago(before(2 * 86_400), now), "2d");
        assert_eq!(ago(before(29 * 86_400), now), "4w");
        assert_eq!(ago(before(400 * 86_400), now), "1y");
    }

    #[test]
    fn short_says_the_time_today_the_day_this_year_else_the_month() {
        let now = at(2026, 11, 13, 12, 0);

        assert_eq!(short(&at(2026, 11, 13, 9, 5), &now), "09:05");
        assert_eq!(short(&at(2026, 3, 4, 9, 5), &now), "4 Mar");
        assert_eq!(short(&at(2025, 12, 31, 9, 5), &now), "Dec 2025");
    }

    #[test]
    fn in_full() {
        let then = at(2026, 11, 5, 14, 5);

        assert_eq!(long(&then), "Thu 5 Nov 2026, 14:05");
        assert_eq!(day(&then), "Thu 5 Nov 2026");
        assert_eq!(clock(&then), "14:05");
    }
}
