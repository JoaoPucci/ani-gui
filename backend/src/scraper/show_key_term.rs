//! The Kitsu-searchable words a history row's show key carries — split
//! from [`super`] so each file stays inside the complexity ratchet's
//! per-file bar. Re-exported from [`crate::scraper::provider`].

use super::{slug_words, ShowKey};

/// The Kitsu-searchable words a history row's show key carries — the
/// slug's hyphenated words, its provider label and numeric id left
/// off (`hianime:jojos-bizarre-adventure-2000-5182` → `jojos bizarre
/// adventure 2000`) — for the picker's search beside the row's saved
/// title, so the renderer never reads a slug. `None` for a key whose
/// words are not spelled as both providers spell a slug, in lower-case
/// ASCII letters, digits and hyphens: an allanime-era id among them.
#[must_use]
pub fn key_search_term(id: &str) -> Option<String> {
    let key = ShowKey::parse(id);
    let words = slug_words(&key.slug)?;
    if !words
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return None;
    }
    let term: Vec<&str> = words.split('-').filter(|w| !w.is_empty()).collect();
    (!term.is_empty()).then(|| term.join(" "))
}

#[cfg(test)]
#[path = "show_key_term_test.rs"]
mod tests;
