//! The year a title match is keyed by, beside its title.
//!
//! A Continue row's guess takes the hit from the year the row names
//! first (the history route's `key_year`, or a year in the row's own
//! title), so two rows titled alike but naming different years — the
//! provider's "JoJo's Bizarre Adventure" from 2000 and from 2012 —
//! guess different shows. Their stored guesses are kept apart by the
//! year: the key's title carries it as a trailing "(2000)", the way a
//! title that names its year already does.

/// The title a title match is keyed by: `title`, with " (<year>)"
/// after it when the row names a year the title does not already end
/// with.
#[must_use]
pub(crate) fn keyed_title(title: &str, year: Option<u32>) -> String {
    match year {
        Some(y) if trailing_year(title) != Some(y) => format!("{title} ({y})"),
        _ => title.to_string(),
    }
}

/// The year a history row's own title names: a trailing "(2009)", on
/// the title as written or on the title less a legacy "(N episodes)"
/// tail — "Show (12 episodes) (2009)" and "Show (2009) (12 episodes)"
/// both name 2009 — from 1960 to 2039, as a show key's year is read.
#[must_use]
pub(crate) fn title_year(title: &str) -> Option<u32> {
    let in_range = |y: &u32| (1960..2040).contains(y);
    trailing_year(title).filter(in_range).or_else(|| {
        trailing_year(crate::commands::history_title_tail::without_episode_tail(
            title,
        ))
        .filter(in_range)
    })
}

/// The four digits a title's trailing parenthesized group holds, spaces
/// inside it aside.
fn trailing_year(title: &str) -> Option<u32> {
    let body = title.trim_end().strip_suffix(')')?;
    let inner = body[body.rfind('(')? + 1..].trim();
    (inner.len() == 4 && inner.bytes().all(|b| b.is_ascii_digit()))
        .then(|| inner.parse().ok())
        .flatten()
}

#[cfg(test)]
#[path = "title_match_year_test.rs"]
mod tests;
