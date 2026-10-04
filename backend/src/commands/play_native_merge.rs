//! Stitching a split entry's part listings into one, in the entry's
//! own numbering. Split from `play_native_split` for the per-file
//! complexity bar; its cases stay with the split module's.

use crate::scraper::provider::EpisodeRef;

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

/// The parts' listings as one, in the entry's own numbering: each
/// part normalised to per-entry numbers, then shifted past every part
/// before it. Episode ids are kept, so a stitched row still streams
/// from the part that lists it.
///
/// Slots are numbered the way a single listing numbers them — the
/// row's position, here across every part — so each row has one of
/// its own: history stores the slot, and the display stamp maps it
/// back to a tag. The row's identity rides in the tag wherever it
/// differs from the slot, which is where the episode step, the cap
/// and the extras already read it.
#[must_use]
pub(crate) fn merge_parts(parts: &[&[EpisodeRef]]) -> Vec<EpisodeRef> {
    use super::play_native_numbering::{kitsu_episode_cap, numbering_offset};
    let mut out: Vec<EpisodeRef> = Vec::new();
    let mut shift = 0u32;
    for part in parts {
        let offset = numbering_offset(part);
        for e in *part {
            let slot = u32::try_from(out.len() + 1).unwrap_or(u32::MAX);
            let shown = display_in_entry(e, offset, shift);
            out.push(EpisodeRef {
                id: e.id,
                number: slot,
                number2: (shown != slot.to_string()).then_some(shown),
            });
        }
        shift = shift.saturating_add(kitsu_episode_cap(part).unwrap_or(0));
    }
    out
}
