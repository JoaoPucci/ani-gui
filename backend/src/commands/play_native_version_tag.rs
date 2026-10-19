//! Reading a title's version tag — split from
//! `play_native_version_copies` for the per-file complexity bar: the
//! bracketed "(Uncensored)" a provider adds to one copy of a season,
//! which the version rule reads to tell copies apart and every reader
//! of a probed candidate's divisions reads past.

use super::play_native_title_grammar::normalized;
use super::play_native_versions::VersionKind;

/// The version tag's word, as a bracket holds it.
const UNCENSORED: &str = "uncensored";

/// `title` read for its version: the title without a bracketed
/// version tag — "(Uncensored)", "[UNCENSORED]", in ASCII or
/// full-width brackets, any case, each character of the tag read in its
/// ASCII form on its own, so "(ｕncensored)" too — compared as written
/// ([`normalized`]: case and spacing aside, catalogue tags kept), and
/// which version the tag names.
pub(crate) fn version_of(title: &str) -> (String, VersionKind) {
    let (out, kind) = split_version_tag(title);
    (normalized(&out), kind)
}

/// `title` without a bracketed version tag, as written otherwise — what
/// the title rule reads a candidate's divisions in, the tag being no
/// more part of the title's structure than a catalogue tag is.
pub(crate) fn without_version_tag(title: &str) -> String {
    split_version_tag(title).0.trim().to_string()
}

/// `title` split into its text without a bracketed version tag and the
/// version the tag names ([`version_of`]). A bracket that holds no tag
/// is read past its opener only, as the catalogue-tag reader reads one,
/// so a tag inside other bracketed words is still found.
fn split_version_tag(title: &str) -> (String, VersionKind) {
    let mut rest = title;
    let mut out = String::new();
    let mut kind = VersionKind::Standard;
    while let Some(open) = rest.find(['(', '[', '（', '［', '【']) {
        let after = &rest[open..];
        let inner_start = after.chars().next().map_or(1, char::len_utf8);
        let tag_close = after
            .find([')', ']', '）', '］', '】'])
            .filter(|&close| is_version_tag(&after[inner_start..close]));
        let Some(close) = tag_close else {
            out.push_str(&rest[..open + inner_start]);
            rest = &rest[open + inner_start..];
            continue;
        };
        kind = VersionKind::Uncensored;
        out.push_str(&rest[..open]);
        out.push(' ');
        let closer = after[close..].chars().next().map_or(1, char::len_utf8);
        rest = &after[close + closer..];
    }
    out.push_str(rest);
    (out, kind)
}

/// Whether the words a bracket holds are the version tag: each character
/// read in its ASCII form on its own, in any case.
fn is_version_tag(inner: &str) -> bool {
    let inner: String = inner
        .trim()
        .chars()
        .map(|c| match c {
            '！'..='～' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect();
    inner.eq_ignore_ascii_case(UNCENSORED)
}
