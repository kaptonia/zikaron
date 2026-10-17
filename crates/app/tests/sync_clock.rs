//! When the rail last synced, said against now (`when::clock_or_day`): the hour and minute on the same day,
//! the month and day too on another one, the day read in the chosen zone (UTC here, the default).

use app::when::clock_or_day;

/// Seconds since the epoch of a UTC moment.
fn at(days: i64, h: i64, m: i64) -> u64 {
    (days * 86_400 + h * 3600 + m * 60) as u64
}

#[test]
fn the_same_day_says_the_clock_and_another_day_the_date_too() {
    let day = 20_739; // 2026-10-13
    assert_eq!(clock_or_day(at(day, 9, 5), at(day, 23, 59)), "09:05", "earlier the same day");
    assert_eq!(clock_or_day(at(day, 23, 59), at(day + 1, 0, 1)), "10-13 23:59", "just before midnight, read just after");
    assert_eq!(clock_or_day(at(day, 0, 0), at(day, 0, 0)), "00:00", "the same moment");
    assert_eq!(clock_or_day(at(day - 400, 12, 0), at(day, 12, 0)), "09-08 12:00", "more than a year back still says month and day");
}
