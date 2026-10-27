//! Kitsu's chains of parts behind the episode map's evidence: the
//! parts filed under one tracker entry, read forward. The prequel walks
//! are in `tracker_map_prequels.rs`. Split from
//! `tracker_map_evidence.rs` so each file stays inside the CRAP gate's
//! per-file bar.

use super::kitsu_side::single_relation;
use super::*;

/// How many hops a chain is followed, on the tracker or on Kitsu, and
/// in either direction: forward through sequels, or back through
/// prequels to the entry a show is filed under and the first part
/// under it. The longest real chain found across Kitsu's catalogue has
/// six segments; a chain still short of the count after this many is
/// not one the rules can settle.
pub(super) const MAX_HOPS: usize = 6;

/// Kitsu's chain of parts from `root`: each the single sequel of the
/// one before, resolving on the tracker to nothing or to `native`
/// itself — never to an entry of its own — followed until the counts
/// reach `tracker_total` or the chain ends.
pub(super) async fn kitsu_parts(
    ask: &MapAsk<'_>,
    root: KitsuShow,
    native: ProviderMediaId,
    tracker_total: u32,
) -> Result<Vec<KitsuShow>> {
    let mut covered = root.episodes.unwrap_or(0);
    let mut parts = vec![root];
    for _ in 0..MAX_HOPS {
        if covered >= tracker_total {
            break;
        }
        let last = parts.last().map(|p| p.id.clone()).unwrap_or_default();
        let Some(sequel) = single_relation(ask.state, &last, "sequel").await? else {
            break;
        };
        let own_entry = crate::commands::account::resolve_native_media_id(
            ask.state,
            ask.kind,
            &sequel.id,
            ask.anilist_base,
        )
        .await?;
        if own_entry.is_some_and(|own| own != native) {
            break;
        }
        let part = sequel;
        covered = covered.saturating_add(part.episodes.unwrap_or(0));
        let more = part.episodes.is_some();
        parts.push(part);
        if !more {
            break;
        }
    }
    Ok(parts)
}

/// Where `parts` reach `total` exactly, at least two of them, file the
/// next sequel among the entries mapped to the entry when it resolves to
/// `native` itself: a part mapped to the entry past the parts that reach
/// its total says they are not the whole of it. Kitsu's pages of the
/// entries mapped to the entry ([`super::owners`]) list it only where
/// one of the ids they are read by maps it, so it is filed here too.
pub(super) async fn file_mapped_past(
    ask: &MapAsk<'_>,
    parts: &[KitsuShow],
    native: ProviderMediaId,
    total: u32,
    sharing: &mut Vec<KitsuShow>,
) -> Result<()> {
    let sum = parts
        .iter()
        .try_fold(0u32, |sum, p| p.episodes.and_then(|n| sum.checked_add(n)));
    if parts.len() < 2 || sum != Some(total) {
        return Ok(());
    }
    if let Some(past) = mapped_past_total(ask, parts, native).await? {
        if !sharing.iter().any(|mapped| mapped.id == past.id) {
            sharing.push(past);
        }
    }
    Ok(())
}

/// The single sequel of the last of `parts` when it resolves to
/// `native` itself.
async fn mapped_past_total(
    ask: &MapAsk<'_>,
    parts: &[KitsuShow],
    native: ProviderMediaId,
) -> Result<Option<KitsuShow>> {
    let last = parts.last().map(|p| p.id.clone()).unwrap_or_default();
    let Some(sequel) = single_relation(ask.state, &last, "sequel").await? else {
        return Ok(None);
    };
    let entry = crate::commands::account::resolve_native_media_id(
        ask.state,
        ask.kind,
        &sequel.id,
        ask.anilist_base,
    )
    .await?;
    Ok((entry == Some(native)).then_some(sequel))
}
