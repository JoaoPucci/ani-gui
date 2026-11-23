//! AniList banners remembered per Kitsu id. The trending feed's one
//! AniList request already carries every entry's `bannerImage`; this
//! keeps each one under the entry's Kitsu id so a detail page of a
//! listed show can carry it with no AniList request of its own. Nothing
//! here makes a network call.

use crate::cache::ttl::ANILIST_BANNER_TTL;
use crate::cache::{meta_cache_delete, meta_cache_get, meta_cache_put, SqlitePool};
use crate::meta::kitsu::KitsuAnimeRef;

fn key(kitsu_id: &str) -> String {
    format!("{}{kitsu_id}", crate::cache::families::ANILIST_BANNER)
}

/// Remember the banner of every ref that carries one, and forget it
/// for every ref AniList answered without one. Cache write failures
/// are ignored: the banner is an enhancement.
pub(crate) fn remember(pool: &SqlitePool, refs: &[KitsuAnimeRef]) {
    for r in refs {
        let _ = match &r.anilist_banner {
            Some(url) => meta_cache_put(pool, &key(&r.id), url, ANILIST_BANNER_TTL.as_secs()),
            None => meta_cache_delete(pool, &key(&r.id)),
        };
    }
}

fn warmed_key(kitsu_id: &str) -> String {
    format!("{}{kitsu_id}", crate::cache::families::ANILIST_STRIP_BANNER)
}

/// Remember what a Continue strip's warm found for a show's banner: the
/// banner, or `None` where AniList's media has none. Kept apart from the
/// trending memo: it is read only where the show's detail has no cover
/// of its own ([`warmed`]), so a show Kitsu has a cover for shows no
/// AniList banner it did not before.
pub(crate) fn remember_warmed(pool: &SqlitePool, kitsu_id: &str, banner: Option<&str>) {
    let _ = meta_cache_put(
        pool,
        &warmed_key(kitsu_id),
        banner.unwrap_or(""),
        ANILIST_BANNER_TTL.as_secs(),
    );
}

/// What a strip's warm found for a show's banner: `Some(None)` where
/// AniList has none, `None` where the warm found nothing to say — no
/// row, or a banner since found gone.
pub(crate) fn warmed(pool: &SqlitePool, kitsu_id: &str) -> Option<Option<String>> {
    let found = meta_cache_get(pool, &warmed_key(kitsu_id)).ok().flatten()?;
    if found.is_empty() {
        return Some(None);
    }
    if crate::meta::images::is_dead(pool, &found) {
        let _ = meta_cache_delete(pool, &warmed_key(kitsu_id));
        return None;
    }
    Some(Some(found))
}

/// The banner remembered for a Kitsu id, if any. A remembered URL the
/// image proxy has since found gone is forgotten here rather than
/// handed out again.
pub(crate) fn remembered(pool: &SqlitePool, kitsu_id: &str) -> Option<String> {
    let url = meta_cache_get(pool, &key(kitsu_id)).ok().flatten()?;
    if crate::meta::images::is_dead(pool, &url) {
        let _ = meta_cache_delete(pool, &key(kitsu_id));
        return None;
    }
    Some(url)
}

/// Set `detail.anilist_banner` from a remembered row, when there is one
/// and the ref carries none.
pub(crate) fn attach(pool: &SqlitePool, detail: &mut KitsuAnimeRef) {
    if detail.anilist_banner.is_none() {
        detail.anilist_banner = remembered(pool, &detail.id);
    }
}

#[cfg(test)]
#[path = "anilist_banner_memo_test.rs"]
mod tests;
