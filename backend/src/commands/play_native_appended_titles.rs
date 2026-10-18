//! How the appended-extras rule compares titles ([`super`]).

/// A title reduced to what tells two titles apart: its letters and
/// digits, lowercased. Case, spacing and punctuation are how two
/// catalogues write one title differently.
fn reduced(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether two titles are one title as the catalogues write it.
#[must_use]
pub fn same_title(a: &str, b: &str) -> bool {
    let a = reduced(a);
    !a.is_empty() && a == reduced(b)
}

/// Whether a title is only a number: "Episode 13", "EP 13", "13".
pub(super) fn is_generic(title: &str) -> bool {
    let r = reduced(title);
    let digits = r
        .strip_prefix("episode")
        .or_else(|| r.strip_prefix("ep"))
        .unwrap_or(&r);
    digits.chars().all(|c| c.is_ascii_digit())
}
