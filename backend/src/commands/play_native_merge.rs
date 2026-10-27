//! Stitching a split entry's part listings into one, in the entry's
//! own numbering. Split from `play_native_split` for the per-file
//! complexity bar; its cases stay with the split module's.

use crate::scraper::provider::EpisodeRef;

#[path = "play_native_merge_former.rs"]
mod former;
#[cfg(test)]
pub(crate) use former::carries_tag;
pub(crate) use former::{
    former_number, former_on_replay, former_tags_of, mark_offset_recaps, names_a_row_now,
};

/// A fractional tag moved `shift` episodes later: "1.5" by 2 is "3.5".
fn shifted_fraction(tag: &str, shift: u32) -> String {
    match tag.split_once('.') {
        Some((int, frac)) => match int.parse::<u32>() {
            Ok(n) => format!("{}.{frac}", n.saturating_add(shift)),
            Err(_) => tag.to_string(),
        },
        None => tag.to_string(),
    }
}

/// A row's identity in the entry's numbering: its part's per-entry
/// number moved `shift` episodes later — an integer for a regular
/// episode, the fractional tag for a recap.
fn display_in_entry(e: &EpisodeRef, offset: u32, shift: u32) -> String {
    use super::play_native_numbering::per_entry_fraction;
    let per_entry = |n: u32| n.saturating_sub(offset).saturating_add(shift).to_string();
    match e.number2.as_deref() {
        None => per_entry(e.number),
        Some(t) => match t.parse::<f64>() {
            Ok(v) if v.fract() == 0.0 && v >= 0.0 => per_entry(v as u32),
            _ => shifted_fraction(&per_entry_fraction(t, offset), shift),
        },
    }
}

/// The tag an earlier version gave a stitched row, when it differs
/// from `shown`, the one it has now. That version passed a part's
/// fraction at its offset (a cumulative part's recap before its first
/// episode, "12.5" under offset 12) through untranslated before
/// shifting it, so the recap came out a part later ("24.5"); and it
/// gave a recap the tag of one before it between the same episodes,
/// which `shown` no longer repeats ([`own_fraction`]). `in_entry` is
/// the row's place in the entry's numbering before that.
fn former_display(
    e: &EpisodeRef,
    offset: u32,
    shift: u32,
    in_entry: &str,
    shown: &str,
) -> Option<String> {
    let t = e.number2.as_deref()?;
    let (int, _) = t.split_once('.')?;
    let at_offset = offset != 0 && int.parse::<u32>().ok()? == offset;
    let former = if at_offset {
        shifted_fraction(t, shift)
    } else {
        in_entry.to_string()
    };
    (former != shown).then_some(former)
}

/// A fraction a row before it in the stitched listing already carries
/// moves halfway on toward the next episode — "12.5" taken, "12.75" —
/// until no row before it has it, so each fraction names one row. A
/// whole number, or a tag that is not a number, is left as it is.
fn own_fraction(tag: String, taken: &[f64]) -> String {
    let Ok(mut v) = tag.parse::<f64>() else {
        return tag;
    };
    if v.fract() == 0.0 || !taken.contains(&v) {
        return tag;
    }
    let next = v.floor() + 1.0;
    while taken.contains(&v) {
        v = (v + next) / 2.0;
    }
    v.to_string()
}

/// The parts' listings as one, in the entry's own numbering: each
/// part normalised to per-entry numbers, then shifted past every part
/// before it. Episode ids are kept, so a stitched row still streams
/// from the part that lists it.
///
/// Slots are numbered the way a single listing numbers them — the
/// row's position, here across every part — so each row has one of
/// its own, as the resolve's episode step expects. The row's identity rides in the tag wherever it
/// differs from the slot, which is where the episode step, the cap
/// and the extras already read it.
#[must_use]
pub(crate) fn merge_parts(parts: &[&[EpisodeRef]]) -> Vec<EpisodeRef> {
    use super::play_native_numbering::{kitsu_episode_cap, numbering_offset};
    let mut out: Vec<EpisodeRef> = Vec::new();
    let mut fractions: Vec<f64> = Vec::new();
    let mut shift = 0u32;
    for part in parts {
        let offset = numbering_offset(part);
        for e in *part {
            let slot = u32::try_from(out.len() + 1).unwrap_or(u32::MAX);
            let in_entry = display_in_entry(e, offset, shift);
            let shown = own_fraction(in_entry.clone(), &fractions);
            fractions.extend(shown.parse::<f64>().ok().filter(|v| v.fract() != 0.0));
            let former_tag = former_display(e, offset, shift, &in_entry, &shown);
            out.push(EpisodeRef {
                id: e.id,
                number: slot,
                number2: (shown != slot.to_string()).then_some(shown),
                former_tag,
                title: e.title.clone(),
            });
        }
        shift = shift.saturating_add(kitsu_episode_cap(part).unwrap_or(0));
    }
    out
}
