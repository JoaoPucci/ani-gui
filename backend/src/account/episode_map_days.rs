//! Days between full dates, and the slack the chain rule allows the
//! sources about one broadcast. Split from the dates comparison so each
//! file stays inside the CRAP gate's per-file bar.

/// Days since 1970-01-01 of a full `YYYY-MM-DD` date, `None` for
/// anything less.
pub(super) fn days(date: Option<&str>) -> Option<i64> {
    let date = date?;
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return None;
    }
    let y: i64 = date[..4].parse().ok()?;
    let m: i64 = date[5..7].parse().ok()?;
    let d: i64 = date[8..].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// How far the sources may straddle a month boundary about one
/// broadcast and still mean the same day.
pub(super) const DAYS_OF_SLACK: i64 = 2;

/// Whether `later` is no more than [`DAYS_OF_SLACK`] after `earlier`.
pub(super) fn within_slack(earlier: Option<&str>, later: Option<&str>) -> bool {
    matches!((days(earlier), days(later)), (Some(a), Some(b)) if b - a <= DAYS_OF_SLACK)
}

#[cfg(test)]
#[path = "episode_map_dates_test.rs"]
mod tests;
