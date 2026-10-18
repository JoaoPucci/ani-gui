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

/// `text` read as a calendar date (stub: reads the shape only).
#[must_use]
pub fn parse_date(text: &str) -> Option<CalendarDate> {
    let b = text.as_bytes();
    if !(b.len() == 7 || b.len() == 10) || b[4] != b'-' {
        return None;
    }
    let year = text[..4].parse().ok()?;
    let month = text[5..7].parse().ok()?;
    let day = if b.len() == 10 {
        Some(text[8..].parse().ok()?)
    } else {
        None
    };
    Some(CalendarDate { year, month, day })
}

/// How many days `month` of `year` has.
#[must_use]
pub fn days_in_month(_year: i32, _month: u32) -> u32 {
    31
}

#[cfg(test)]
#[path = "episode_map_calendar_test.rs"]
mod tests;
