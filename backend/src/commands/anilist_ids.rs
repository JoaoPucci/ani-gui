//! The one way a command translates ids between AniList and MAL.
//!
//! AniList's `Media` carries both its own id and MAL's, so it is the
//! bridge whenever Kitsu maps a show to one tracker only: a MAL id
//! becomes AniList's `mediaId` for the AniList account, an AniList id
//! becomes the `idMal` the MAL account keys on, and the Watch Later
//! rail asks for many MAL ids at once. Every one of these is a public
//! AniList request against the 30-per-minute budget the rest of the
//! app shares, so they are asked through here.

use std::collections::HashMap;

use crate::app::AppState;
use crate::error::Result;

/// AniList's `mediaId` for a MAL id; `None` when AniList does not
/// index it.
///
/// # Errors
/// Same as [`crate::meta::anilist::media_id_for_mal`].
pub(crate) async fn media_id_for_mal(
    state: &AppState,
    mal_id: u32,
    anilist_base: Option<&str>,
) -> Result<Option<u32>> {
    crate::meta::anilist::media_id_for_mal(&state.meta_http, mal_id, anilist_base).await
}

/// The MAL id AniList records for one of its media; `None` when it
/// records none.
///
/// # Errors
/// Same as [`crate::meta::anilist::mal_id_for_media_id`].
pub(crate) async fn mal_id_for_media_id(
    state: &AppState,
    media_id: u32,
    anilist_base: Option<&str>,
) -> Result<Option<u32>> {
    crate::meta::anilist::mal_id_for_media_id(&state.meta_http, media_id, anilist_base).await
}

/// AniList's `mediaId` for each of many MAL ids; ids AniList does not
/// index are absent from the map.
///
/// # Errors
/// Same as [`crate::meta::anilist::media_ids_for_mals`].
pub(crate) async fn media_ids_for_mals(
    state: &AppState,
    mal_ids: &[u32],
    anilist_base: Option<&str>,
) -> Result<HashMap<u32, u32>> {
    crate::meta::anilist::media_ids_for_mals(&state.meta_http, mal_ids, anilist_base).await
}
