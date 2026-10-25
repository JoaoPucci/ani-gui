//! Kitsu's chains of parts behind the episode map's evidence: the
//! parts filed under one tracker entry, and the prequel an unresolved
//! part is filed through. Split from `tracker_map_evidence.rs` so each
//! file stays inside the CRAP gate's per-file bar.

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

/// The single sequel of the last of `parts` when it resolves to
/// `native` itself: a part mapped to the entry past the parts that reach
/// its total, which says they are not the whole of it. Kitsu's page of
/// the entries mapped to the tracker's id need not list it — it may be
/// mapped through the other tracker's id — so the gatherer files it
/// among them.
pub(super) async fn mapped_past_total(
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

/// The first Kitsu part filed under `native`: `kitsu` itself, or the
/// earliest of its single prequels, up the chain, that resolves to
/// `native` too. Prequels that resolve to nothing are walked through,
/// as [`kitsu_parts`] takes them forward; one that resolves to another
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
