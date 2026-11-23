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

/// `name` with the characters Windows refuses in a file name taken out,
/// so a title becomes a name on every platform the app writes to: on
/// NTFS a colon names a hidden stream of the file named up to it, and
/// the rest are refused outright. A colon reads as " - " between the
/// words it joined; `/`, `\` and `|` become a hyphen; `?`, `"`, `<`,
/// `>`, `*` and control characters go. Everything else is kept, so a
/// title Windows already accepts is its own name.
#[must_use]
pub(crate) fn file_safe(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let mut after_colon = false;
    for c in name.chars() {
        let kept = match c {
            ':' => {
                out.truncate(out.trim_end().len());
                out.push_str(if out.is_empty() { "-" } else { " -" });
                after_colon = true;
                continue;
            }
            '/' | '\\' | '|' => '-',
            '?' | '"' | '<' | '>' | '*' => continue,
            c if (c as u32) < 0x20 => continue,
            c => c,
        };
        if after_colon {
            if kept.is_whitespace() {
                continue;
            }
            out.push(' ');
            after_colon = false;
        }
        out.push(kept);
    }
    out
}

#[cfg(test)]
#[path = "download_names_test.rs"]
mod tests;
