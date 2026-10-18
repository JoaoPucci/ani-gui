//! The spelling a sidecar takes beside the media.
//!
//! Whether two names are one file is the filesystem's question, and
//! the packaged platforms answer it by different tables: Windows'
//! folds case by its own, which calls Greek sigma and final sigma one
//! name where this program's fold does not, and a table can only be
//! wrong one entry at a time — the target lock in `download` records
//! three such entries. A name the app gives a file therefore stays
//! inside the one alphabet every table agrees on, ASCII letters,
//! digits and hyphens, so that two names it tells apart are two files
//! everywhere.

/// The portable spelling of a subtitle tag, or none when the tag
/// leaves nothing of the alphabet: ASCII letters and digits are kept
/// as they are — a tag's own spelling leads, `pt-BR` stays `pt-BR` —
/// and every run of anything else becomes one hyphen, with no hyphen
/// leading or trailing.
#[must_use]
pub(crate) fn portable_name(tag: &str) -> Option<String> {
    let mut name = String::with_capacity(tag.len());
    for c in tag.chars() {
        if c.is_ascii_alphanumeric() {
            name.push(c);
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let trimmed = name.trim_matches('-');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// The name an episode's download takes in `dest`: `"{title} Episode
/// {episode}"`, or the one an earlier version gave it, numbered
/// `former`, when a file under that name is already there — that file
/// is the episode.
#[must_use]
pub(crate) fn episode_stem(
    dest: &std::path::Path,
    title: &str,
    episode: &str,
    former: Option<&str>,
) -> String {
    let stem = |n: &str| format!("{title} Episode {n}");
    former
        .map(stem)
        .filter(|s| std::fs::symlink_metadata(dest.join(format!("{s}.mp4"))).is_ok())
        .unwrap_or_else(|| stem(episode))
}

#[cfg(test)]
#[path = "download_names_test.rs"]
mod tests;
