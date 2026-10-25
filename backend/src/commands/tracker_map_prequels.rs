//! Kitsu's prequels behind the episode map's evidence: the first part
//! filed under a tracker entry, and the prequel an unresolved part is
//! filed through. Split from `tracker_map_parts.rs` so each file stays
//! inside the CRAP gate's per-file bar.

use super::kitsu_side::single_relation;
use super::parts_walk::MAX_HOPS;
use super::*;

/// The first Kitsu part filed under `native`: `kitsu` itself, or the
/// earliest of its single prequels, up the chain, that resolves to
/// `native` too. Prequels that resolve to nothing are walked through,
/// as [`super::parts_walk::kitsu_parts`] takes them forward; one that resolves to another
/// entry ends the walk.
pub(super) async fn first_part_under(
    ask: &MapAsk<'_>,
    kitsu: &KitsuShow,
    native: ProviderMediaId,
) -> Result<KitsuShow> {
    let mut root = kitsu.clone();
    let mut current = kitsu.id.clone();
    for _ in 0..MAX_HOPS {
        let Some(prequel) = single_relation(ask.state, &current, "prequel").await? else {
            break;
        };
        let resolves = crate::commands::account::resolve_native_media_id(
            ask.state,
            ask.kind,
            &prequel.id,
            ask.anilist_base,
        )
        .await?;
        match resolves {
            Some(entry) if entry == native => {
                current = prequel.id.clone();
                root = prequel;
            }
            Some(_) => break,
            None => current = prequel.id,
        }
    }
    Ok(root)
}

/// The first Kitsu entry up the single-prequel chain from `kitsu_id`
/// that resolves on the tracker, with the entry it resolves to.
pub(super) async fn root_through_prequels(
    ask: &MapAsk<'_>,
    kitsu_id: &str,
) -> Result<Option<(KitsuShow, ProviderMediaId)>> {
    let mut current = kitsu_id.to_owned();
    for _ in 0..MAX_HOPS {
        let Some(prequel) = single_relation(ask.state, &current, "prequel").await? else {
            return Ok(None);
        };
        let show = prequel;
        if let Some(native) = crate::commands::account::resolve_native_media_id(
            ask.state,
            ask.kind,
            &show.id,
            ask.anilist_base,
        )
        .await?
        {
            return Ok(Some((show, native)));
        }
        current = show.id;
    }
    Ok(None)
}
