//! How a Continue row's guess reads titles: a title's words, a hit's
//! names, and the rules that read them alone — the episode reach, the
//! series a title names, its first word.

use crate::meta::kitsu::KitsuAnimeRef;

/// A row's title words: runs of ASCII letters and digits, lowercase,
/// the articles left out.
pub(super) fn words(title: &str) -> Vec<String> {
    title
        .to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
        .filter(|w| !w.is_empty() && !matches!(*w, "the" | "a" | "an"))
        .map(str::to_owned)
        .collect()
}

/// Every title a hit carries: canonical, localized, abbreviated.
pub(super) fn names(hit: &KitsuAnimeRef) -> impl Iterator<Item = &str> {
    std::iter::once(hit.canonical_title.as_str())
        .chain(hit.titles.values().map(String::as_str))
        .chain(hit.abbreviated_titles.iter().map(String::as_str))
}

/// Whether a row at `episode` is past what `count` episodes hold under
/// the provider's numbering: past twice the count and two more. A hit
/// with no count is never past.
#[must_use]
pub fn past_reach(episode: f64, count: Option<u32>) -> bool {
    count.is_some_and(|n| episode > f64::from(n) * 2.0 + 2.0)
}

/// Whether a title names a series: it carries "TV" as a word.
#[must_use]
pub fn names_series(title: &str) -> bool {
    words(title).iter().any(|w| w == "tv")
}

/// Whether a Kitsu `subtype` is a series: TV, or a web series (ONA).
#[must_use]
pub fn is_series(subtype: Option<&str>) -> bool {
    subtype.is_some_and(|s| s.eq_ignore_ascii_case("tv") || s.eq_ignore_ascii_case("ona"))
}

/// Whether one of `hit`'s titles, or its slug's words, carries the
/// first word of `title`. A title with no words is carried by every
/// hit.
#[must_use]
pub fn carries_first_word(title: &str, hit: &KitsuAnimeRef) -> bool {
    let Some(lead) = words(title).into_iter().next() else {
        return true;
    };
    let slug = hit.slug.as_deref().map(|s| s.replace('-', " "));
    let carried = names(hit)
        .chain(slug.as_deref())
        .any(|t| words(t).contains(&lead));
    carried
}
