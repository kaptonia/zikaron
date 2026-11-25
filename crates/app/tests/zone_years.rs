use app::platform::Zone;
use app::when::offset_under;

fn noon(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) * 86_400 + 12 * 3600
}

const HOUR: i64 = 3600;
const BEFORE: &str = "EST5EDT,M4.1.0,M10.5.0";
const AFTER: &str = "EST5EDT,M3.2.0,M11.1.0";

fn eastern() -> Zone {
    Zone::ByYear(vec![(1970, BEFORE.to_string()), (2007, AFTER.to_string())])
}

#[test]
fn a_moment_reads_by_its_own_years_rule() {
    let z = eastern();
    assert_eq!(offset_under(&z, noon(2006, 3, 20)), Some(-5 * HOUR), "2006-03-20 by 2006's rule");
    assert_eq!(offset_under(&z, noon(2007, 3, 20)), Some(-4 * HOUR), "2007-03-20 by 2007's rule");
    assert_eq!(offset_under(&z, noon(2006, 11, 2)), Some(-5 * HOUR), "2006-11-02 by 2006's rule");
    assert_eq!(offset_under(&z, noon(2007, 11, 2)), Some(-4 * HOUR), "2007-11-02 by 2007's rule");
    for y in [2006, 2007] {
        assert_eq!(offset_under(&z, noon(y, 7, 1)), Some(-4 * HOUR), "{y}-07-01");
        assert_eq!(offset_under(&z, noon(y, 1, 15)), Some(-5 * HOUR), "{y}-01-15");
    }
    let today = Zone::Rule(AFTER.to_string());
    assert_eq!(offset_under(&today, noon(2006, 3, 20)), Some(-4 * HOUR), "one rule has no years");
}

#[test]
fn one_rule_keeps_its_daylight_time() {
    let ny = Zone::Rule(AFTER.to_string());
    assert_eq!(offset_under(&ny, noon(2026, 7, 1)), Some(-4 * HOUR), "summer");
    assert_eq!(offset_under(&ny, noon(2026, 1, 15)), Some(-5 * HOUR), "winter");
    let sydney = Zone::Rule("AEST-10AEDT,M10.1.0,M4.1.0/3".to_string());
    assert_eq!(offset_under(&sydney, noon(2026, 1, 15)), Some(11 * HOUR), "southern summer");
    assert_eq!(offset_under(&sydney, noon(2026, 7, 1)), Some(10 * HOUR), "southern winter");
    assert_eq!(offset_under(&Zone::Rule("<+08>-8".to_string()), noon(2026, 7, 1)), Some(8 * HOUR), "no daylight time");
}

#[test]
fn the_tables_ends_hold_before_and_after_it() {
    let z = Zone::ByYear(vec![(2000, BEFORE.to_string()), (2007, AFTER.to_string())]);
    assert_eq!(offset_under(&z, noon(1990, 3, 20)), Some(-5 * HOUR), "before the first year: the first rule");
    assert_eq!(offset_under(&z, noon(2006, 3, 20)), Some(-5 * HOUR), "between: the latest at or before");
    assert_eq!(offset_under(&z, noon(2050, 3, 20)), Some(-4 * HOUR), "after the last year: the last rule");
    let one = Zone::ByYear(vec![(2007, AFTER.to_string())]);
    assert_eq!(offset_under(&one, noon(1999, 3, 20)), Some(-4 * HOUR));
}

#[test]
fn the_year_is_the_one_where_the_moment_is_shown() {
    let z = Zone::ByYear(vec![(2000, "<+10>-10".to_string()), (2011, "<+11>-11".to_string())]);
    let eve = noon(2010, 12, 31);
    assert_eq!(offset_under(&z, eve), Some(10 * HOUR), "22:00 on 2010-12-31 by 2010's clock: 2010's");
    assert_eq!(offset_under(&z, eve + 2 * HOUR - 1), Some(10 * HOUR), "23:59:59 by 2010's clock: 2010's");
    assert_eq!(offset_under(&z, eve + 2 * HOUR), Some(11 * HOUR), "midnight by 2010's clock: the year turned, 2011's");
    let w = Zone::ByYear(vec![(2000, "<-10>10".to_string()), (2011, "<-11>11".to_string())]);
    let dawn = noon(2011, 1, 1) - 12 * HOUR;
    assert_eq!(offset_under(&w, dawn + 10 * HOUR - 1), Some(-10 * HOUR), "23:59:59 on 2010-12-31 by 2010's clock: 2010's");
    assert_eq!(offset_under(&w, dawn + 10 * HOUR), Some(-11 * HOUR), "midnight by 2010's clock: the year turned, 2011's");
}

#[test]
fn a_table_that_does_not_read_gives_no_offset() {
    assert_eq!(offset_under(&Zone::ByYear(Vec::new()), noon(2007, 3, 20)), None, "an empty table");
    let bad = Zone::ByYear(vec![(1970, BEFORE.to_string()), (2007, "not a rule".to_string())]);
    assert_eq!(offset_under(&bad, noon(2006, 3, 20)), None, "one year that does not parse: the table does not read");
    assert_eq!(offset_under(&Zone::Rule(String::new()), 0), None, "an empty rule");
    assert_eq!(offset_under(&Zone::File(b"not a zone file".to_vec()), 0), None, "not a zone file");
}

#[test]
fn the_seconds_ends_neither_wrap_nor_stop() {
    let z = eastern();
    assert_eq!(offset_under(&z, 0), Some(-5 * HOUR), "the epoch: 1970's rule, winter");
    assert_eq!(offset_under(&z, -1), Some(-5 * HOUR), "before the epoch: read as the epoch");
    assert_eq!(offset_under(&z, i64::MIN), Some(-5 * HOUR), "the least second");
    assert_eq!(offset_under(&z, i64::MAX), Some(-5 * HOUR), "the greatest second");
    let late = Zone::ByYear(vec![(1970, "<-01>1<+00>,0/0,J365/23".to_string())]);
    assert!(offset_under(&late, i64::MAX).is_some(), "a last-day switch at the greatest second");
}
