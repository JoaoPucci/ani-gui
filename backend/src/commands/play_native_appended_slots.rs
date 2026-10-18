//! The slots the appended-extras rule tags, and a history row's slot
//! read back as its extra ([`super`]).

use super::Remembered;
use crate::scraper::provider::EpisodeRef;

/// The extra a history row's episode names under `reading`, when the
/// row is for the show the reading was proved on (`show_key`, as the
/// history file keys it) and its episode is the slot of a row the
/// reading tagged: an earlier version recorded Noragami's first OAD as
/// episode 13, which reads back as `12.1`.
#[must_use]
pub fn extra_for_slot(reading: &Remembered, show_key: &str, ep_no: &str) -> Option<String> {
    let key = crate::scraper::provider::ShowKey::parse(show_key);
    if key.provider.is_none() || key.slug != reading.slug {
        return None;
    }
    let slot: u32 = ep_no.trim().parse().ok()?;
    let i = slot.checked_sub(reading.count).filter(|i| *i > 0)?;
    (usize::try_from(i).ok()? <= reading.ids.len()).then(|| format!("{}.{i}", reading.count))
}

/// The provider's ids of the rows past `count`, in order.
pub(super) fn past_ids(listing: &[EpisodeRef], count: u32) -> Vec<u64> {
    let at = usize::try_from(count).unwrap_or(usize::MAX);
    listing.iter().skip(at).map(|e| e.id).collect()
}

/// `listing` with its rows past `count` tagged `count.1`, `count.2`,
/// … in listing order, each row otherwise the row it was.
#[must_use]
pub fn tag_past(listing: &[EpisodeRef], count: u32) -> Vec<EpisodeRef> {
    let at = usize::try_from(count).unwrap_or(usize::MAX);
    let mut out = listing.to_vec();
    for (row, i) in out.iter_mut().skip(at).zip(1u32..) {
        row.number2 = Some(format!("{count}.{i}"));
    }
    out
}
