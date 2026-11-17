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

/// The name an episode's download takes in `dest`: `"{title} Episode
/// {episode}"` with the title made [`file_safe`], or the one an
/// earlier version gave it — numbered `former`, or spelled with the
/// title as the provider gave it — when a file under that name is
/// already there: that file is the episode.
///
/// The provider's spelling is looked for only off Windows. Earlier
/// versions saved it as a real file only where the filesystem accepts
/// the colon; on Windows the same path names a stream of the empty
/// file a failed download left, and finding that stream would hand
/// the broken name out again.
#[must_use]
pub(crate) fn episode_stem(
    dest: &std::path::Path,
    title: &str,
    episode: &str,
    former: Option<&str>,
) -> String {
    let safe = file_safe(title);
    let stem = |t: &str, n: &str| format!("{t} Episode {n}");
    let on_disk = |s: &String| std::fs::symlink_metadata(dest.join(format!("{s}.mp4"))).is_ok();
    let mut earlier: Vec<String> = Vec::new();
    if let Some(n) = former {
        earlier.push(stem(&safe, n));
    }
    // A title with a slash was never an earlier download's name, and
    // joining it as a path can reach outside `dest`.
    if !cfg!(windows) && safe != title && !title.contains('/') {
        if let Some(n) = former {
            earlier.push(stem(title, n));
        }
        earlier.push(stem(title, episode));
    }
    earlier
        .into_iter()
        .find(on_disk)
        .unwrap_or_else(|| not_a_device(stem(&safe, episode)))
}

/// `name` with its first dot made a hyphen when the part before that
/// dot is a device name, which Windows 10 opens as the device whatever
/// follows the dot. Any other name is returned as it is.
fn not_a_device(name: String) -> String {
    const DEVICES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
    let Some(dot) = name.find('.') else {
        return name;
    };
    let base = name[..dot].trim_end().to_ascii_uppercase();
    let numbered = |prefix: &str| {
        base.strip_prefix(prefix)
            .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit() && n != "0")
    };
    if DEVICES.contains(&base.as_str()) || numbered("COM") || numbered("LPT") {
        let mut renamed = name;
        renamed.replace_range(dot..=dot, "-");
        renamed
    } else {
        name
    }
}

#[cfg(test)]
#[path = "download_names_test.rs"]
mod tests;
