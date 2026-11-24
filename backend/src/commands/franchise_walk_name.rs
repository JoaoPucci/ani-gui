//! The franchise's name, decided with the walk's answer: the title of
//! the main story's first series by start (`franchise_walk_series.rs`;
//! films, pilots, specials and one-episode OVAs or ONAs before it are
//! skipped), else of its first entry by start. Kitsu's format suffix
//! ("(TV)", "(Movie)", "(OVA)", "(ONA)", "(Special)", any case) is
//! dropped, unless another entry of the franchise, in any section,
//! would then read the same, ignoring case: the suffix is what tells
//! the two apart. A title that is nothing but a suffix keeps it. The
//! franchise page's heading and artwork, and the front card of the
//! strip's poster stack, come from that entry.

use super::series::is_series;
use crate::meta::kitsu::FranchiseEntry;

/// The format suffixes Kitsu gives a title.
const SUFFIXES: [&str; 5] = ["tv", "movie", "ova", "ona", "special"];

/// The entry of `entries` (in start order) the franchise is named
/// after.
#[must_use]
pub(super) fn name_entry(entries: &[FranchiseEntry]) -> Option<&FranchiseEntry> {
    entries.iter().find(|e| is_series(e)).or(entries.first())
}

/// `title` without a trailing format suffix.
#[must_use]
pub(super) fn without_suffix(title: &str) -> &str {
    let Some(open) = title.strip_suffix(')').and_then(|t| t.rfind('(')) else {
        return title;
    };
    let inner = &title[open + 1..title.len() - 1];
    if SUFFIXES.iter().any(|s| inner.eq_ignore_ascii_case(s)) {
        title[..open].trim_end()
    } else {
        title
    }
}

/// `title` without its suffix, unless one of `others` reads the same
/// once its own suffix is dropped, ignoring case.
#[must_use]
pub(super) fn display_name<'a>(title: &str, others: impl IntoIterator<Item = &'a str>) -> String {
    let bare = without_suffix(title);
    let key = bare.to_lowercase();
    let clash = others
        .into_iter()
        .any(|o| without_suffix(o).to_lowercase() == key);
    if clash || bare.trim().is_empty() {
        title
    } else {
        bare
    }
    .to_string()
}

/// The franchise's name and the main-story entry it comes from, over
/// the run (`main`, in start order) and the sections `around` it. A run
/// with no entry, which the walk never answers, is named nothing after
/// the anchor.
#[must_use]
pub(super) fn named(
    anchor: &str,
    main: &[FranchiseEntry],
    around: [&[FranchiseEntry]; 4],
) -> (String, String) {
    let Some(e) = name_entry(main) else {
        return (String::new(), anchor.to_string());
    };
    let others = std::iter::once(main)
        .chain(around)
        .flatten()
        .filter(|o| o.id != e.id)
        .map(|o| o.title.as_str());
    (display_name(&e.title, others), e.id.clone())
}
