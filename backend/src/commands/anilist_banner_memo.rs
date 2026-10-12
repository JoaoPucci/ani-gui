//! AniList banners remembered per Kitsu id. The trending feed's one
//! AniList request already carries every entry's `bannerImage`; this
//! keeps each one under the entry's Kitsu id so a detail page of a
//! listed show can carry it with no AniList request of its own. Nothing
//! here makes a network call.

use crate::cache::ttl::ANILIST_BANNER_TTL;
use crate::cache::{meta_cache_get, meta_cache_put, SqlitePool};
use crate::meta::kitsu::KitsuAnimeRef;

fn key(kitsu_id: &str) -> String {
    format!("anilist:banner:v1:k{kitsu_id}")
}

/// Remember the banner of every ref that carries one. Cache write
/// failures are ignored: the banner is an enhancement.
pub(crate) fn remember(pool: &SqlitePool, refs: &[KitsuAnimeRef]) {
    for r in refs {
        if let Some(url) = &r.anilist_banner {
            let _ = meta_cache_put(pool, &key(&r.id), url, ANILIST_BANNER_TTL.as_secs());
        }
    }
}

/// Set `detail.anilist_banner` from a remembered row, when there is one
/// and the ref carries none.
pub(crate) fn attach(pool: &SqlitePool, detail: &mut KitsuAnimeRef) {
    if detail.anilist_banner.is_none() {
        detail.anilist_banner = meta_cache_get(pool, &key(&detail.id)).ok().flatten();
    }
}
