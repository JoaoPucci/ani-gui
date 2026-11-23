//! The names a download gives its files in the working folder, and
//! which download a name found there belongs to.

use std::path::{Path, PathBuf};

/// The working folder's name inside a destination.
pub(crate) const PARTS_DIR: &str = ".ani-gui-parts";

/// Where `dest`'s unfinished files live.
pub(crate) fn parts_dir(dest: &Path) -> PathBuf {
    dest.join(PARTS_DIR)
}

/// What a name in the working folder is to the download that owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Entry {
    /// `<key>.hold`, the file the owner keeps locked.
    Hold,
    /// `<key>-…`, one of the owner's files.
    Part,
}

pub(super) const KEY_PREFIX: &str = ".ani-gui-";
/// The random part of a key: 64 bits, in lowercase hex. Short, because
/// every name a tool derives inside the working folder carries it on
/// top of the destination's path, and Windows paths have a ceiling.
const ID_LEN: usize = 16;

/// The key that owns `name`, and what the name is to it; none for a
/// name this scheme did not give out, which the sweep leaves alone.
///
/// A key is `.ani-gui-<pid>-<16 lowercase hex>`, exactly as [`super::Hold`]
/// spells it, so the parse is syntactic: the hold is the key and
/// `.hold`, a part the key and a hyphen and anything after it.
#[must_use]
pub(crate) fn owner_of(name: &str) -> Option<(&str, Entry)> {
    let rest = name.strip_prefix(KEY_PREFIX)?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = rest[digits..].strip_prefix('-')?;
    let id = rest.get(..ID_LEN)?;
    if !id
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let key_len = KEY_PREFIX.len() + digits + 1 + ID_LEN;
    let (key, tail) = name.split_at(key_len);
    if tail == ".hold" {
        Some((key, Entry::Hold))
    } else if tail.len() > 1 && tail.starts_with('-') {
        Some((key, Entry::Part))
    } else {
        None
    }
}
