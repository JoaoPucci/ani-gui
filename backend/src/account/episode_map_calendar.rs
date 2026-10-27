//! Reading a date the episode map compares: `YYYY-MM-DD`, or `YYYY-MM`
//! where a source knows only the month. A string that names no real
//! calendar date — a thirteenth month, an April 31st — is no date, never
//! one carried into the month after.

/// A date as far as its source knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarDate {
    /// The year.
    pub year: i32,
    /// The month, 1–12.
    pub month: u32,
    /// The day of the month, where the source gives one.
    pub day: Option<u32>,
}

/// `text` read as a calendar date: exactly `YYYY-MM-DD` or `YYYY-MM`,
/// ASCII digits, a month 1–12 and a day the month has.
#[must_use]
pub fn parse_date(text: &str) -> Option<CalendarDate> {
    let b = text.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if !(b.len() == 7 || b.len() == 10) || b[4] != b'-' || !digits(0..4) || !digits(5..7) {
        return None;
    }
    let year: i32 = text[..4].parse().ok()?;
    let month: u32 = text[5..7].parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    let day = if b.len() == 10 {
        if b[7] != b'-' || !digits(8..10) {
            return None;
        }
        let day: u32 = text[8..].parse().ok()?;
        if day == 0 || day > days_in_month(year, month) {
            return None;
        }
        Some(day)
    } else {
        None
    };
    Some(CalendarDate { year, month, day })
}

/// How many days `month` (1–12) of `year` has, in the Gregorian
/// calendar.
#[must_use]
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
#[path = "episode_map_calendar_test.rs"]
mod tests;
