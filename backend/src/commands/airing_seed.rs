//! Batch-seeding airing rows for the shows a home rail warms, one
//! AniList request per id space; split from [`super`] so each file
//! stays inside the CRAP gate's per-file bar.

use super::*;

/// Batch-seed airing rows for many shows: the home-rail warm calls
/// this once, so its probes' writes — a current show's count, a
/// pre-premiere negative — find their schedule in the cache instead
/// of paying one AniList request per show. Shows with an AniList id
/// go in one `Page(media(id_in))` request; shows Kitsu maps to MAL
/// alone go in a second, `idMal_in` one (AniList ANDs the two filters
/// on one `media` field) — one request per id space for a rail of up
/// to 50 shows. Shows whose airing row is still fresh cost nothing,
/// and a show with no mapping has no schedule to seed. Best-effort
/// throughout — a mapping or fetch failure leaves rows unwritten, and
/// any later airing write for the show still cuts the rows written
/// before it.
pub(crate) async fn seed_airing_rows_batch(
    state: &AppState,
    kitsu_ids: &[String],
    anilist_base: Option<&str>,
) {
    let mut by_anilist: Vec<(String, u32)> = Vec::new();
    let mut by_mal: Vec<(String, u32)> = Vec::new();
    for kitsu_id in kitsu_ids {
        let key = format!("airing:v2:{kitsu_id}");
        if matches!(meta_cache_get(&state.cache_pool, &key), Ok(Some(_))) {
            continue;
        }
        let Ok(ids) = state.kitsu.external_ids_for_kitsu_id(kitsu_id).await else {
            continue;
        };
        match (ids.anilist, ids.mal) {
            (Some(anilist_id), _) => by_anilist.push((kitsu_id.clone(), anilist_id)),
            (None, Some(mal_id)) => by_mal.push((kitsu_id.clone(), mal_id)),
            (None, None) => {}
        }
    }
    let client = &state.meta_http;
    if !by_anilist.is_empty() {
        let ids: Vec<u32> = by_anilist.iter().map(|(_, a)| *a).collect();
        let fetched =
            crate::meta::anilist_airing::airing_status_batch(client, &ids, anilist_base).await;
        write_batch(state, &by_anilist, fetched);
    }
    if !by_mal.is_empty() {
        let ids: Vec<u32> = by_mal.iter().map(|(_, m)| *m).collect();
        let fetched =
            crate::meta::anilist_airing::airing_status_batch_by_mal(client, &ids, anilist_base)
                .await;
        write_batch(state, &by_mal, fetched);
    }
}

/// Write each requested show's row from one batch answer, keyed by
/// the id the request addressed it by. A failed request, or an id
/// AniList didn't return, writes nothing.
fn write_batch(
    state: &AppState,
    pairs: &[(String, u32)],
    fetched: Result<std::collections::HashMap<u32, AiringStatus>>,
) {
    let Ok(map) = fetched else {
        return;
    };
    for (kitsu_id, id) in pairs {
        if let Some(status) = map.get(id) {
            write_airing_row(state, kitsu_id, status);
        }
    }
}
