//! The text fields of AniList's evidence: its fuzzy dates and the
//! numbers its streaming-episode titles carry. Split from
//! `anilist_user_evidence.rs` so each file stays inside the CRAP gate's
//! per-file bar.

/// AniList's fuzzy date as `YYYY-MM-DD`, or as much of it as is known
/// (`YYYY-MM`, `YYYY`); `None` without a year, or where the month or day
/// names no real calendar date — an impossible date is no date, never
/// one carried into a neighbouring month.
pub(super) fn fuzzy_date(
    year: Option<u32>,
    month: Option<u32>,
    day: Option<u32>,
) -> Option<String> {
    let year = format!("{:04}", year?);
    let Some(month) = month else {
        return (year.len() == 4).then_some(year);
    };
    let out = match day {
        Some(day) => format!("{year}-{month:02}-{day:02}"),
        None => format!("{year}-{month:02}"),
    };
    crate::account::episode_map_calendar::parse_date(&out).map(|_| out)
}

/// The number a streaming-episode title carries — `"Episode 6.5 -
/// Ekwari"` is `"6.5"`, `"Episode 0 - Prologue"` is `"0"` — or `None`
/// for a title that does not open on `Episode <number>`.
pub(super) fn listed_number_of(title: &str) -> Option<String> {
    let trimmed = title.trim();
    let rest = trimmed
        .strip_prefix("Episode ")
        .or_else(|| trimmed.strip_prefix("episode "))?;
    let number: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!number.is_empty() && number.starts_with(|c: char| c.is_ascii_digit())).then_some(number)
}
