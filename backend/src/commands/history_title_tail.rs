//! The title Continue Watching searched, read back from a row's title
//! — split from `history_forget_titles` for the per-file complexity
//! bar.

/// `title` less a trailing `(N episodes)`, itself optionally followed
/// by `(year)` — the tail rows written before the provider migration
/// carry, which Continue Watching strips before it searches.
pub(crate) fn without_episode_tail(title: &str) -> &str {
    let is_year = |inner: &str| (1..=4).contains(&inner.len()) && all_digits(inner);
    let before_year = strip_paren_tail(title, is_year).unwrap_or(title);
    strip_paren_tail(before_year, is_episode_count)
        .or_else(|| strip_paren_tail(title, is_episode_count))
        .unwrap_or(title)
        .trim()
}

/// `s` less a trailing parenthesized group whose trimmed contents pass
/// `inner`.
fn strip_paren_tail(s: &str, inner: impl Fn(&str) -> bool) -> Option<&str> {
    let body = s.trim_end().strip_suffix(')')?;
    let open = body.rfind('(')?;
    inner(body[open + 1..].trim()).then(|| body[..open].trim_end())
}

/// `N episode` or `N episodes`, any case.
fn is_episode_count(inner: &str) -> bool {
    let Some((count, word)) = inner.split_once(char::is_whitespace) else {
        return false;
    };
    let word = word.trim().to_ascii_lowercase();
    all_digits(count) && (word == "episode" || word == "episodes")
}

/// Whether `s` is one or more ASCII digits.
pub(crate) fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
#[path = "history_title_tail_test.rs"]
mod tests;
