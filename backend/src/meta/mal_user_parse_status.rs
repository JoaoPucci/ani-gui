//! MAL's list-status replies; split from [`super`] so each file stays
//! inside the CRAP gate's per-file bar. Their timestamps go through the
//! ISO-8601 reader in `mal_user_parse_time.rs`.

use super::*;

/// Parse a bare `my_list_status` response (PATCH return body). MAL
/// returns just the updated `list_status` here, no anime metadata —
/// the caller already knows which anime they PATCHed. The trait
/// returns `ListEntry`, so the caller supplies the media id; title
/// is left empty (the cache write-through merges with the row from
/// `list_all` if present).
pub(in crate::meta) fn parse_list_status_response(body: &[u8], media_id: u32) -> Result<ListEntry> {
    #[derive(Deserialize)]
    struct WireListStatus {
        status: String,
        #[serde(default)]
        score: Option<u8>,
        // Sent on every list status: missing, the row says nothing of
        // how far the user is.
        num_episodes_watched: crate::meta::present::Present<u32>,
        #[serde(default)]
        is_rewatching: Option<bool>,
        #[serde(default)]
        updated_at: Option<String>,
    }
    let wire: WireListStatus = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal my_list_status response: {e}"),
    })?;
    let status = ListStatus::from_mal(&wire.status, wire.is_rewatching.unwrap_or(false))
        .ok_or_else(|| AniError::ParseFailed {
            detail: format!("mal my_list_status unknown status: {}", wire.status),
        })?;
    let updated_at_epoch_s = wire.updated_at.as_deref().map_or(0, parse_iso8601_to_epoch);
    let score_0_to_100 = wire.score.filter(|s| *s > 0).map(|s| s.saturating_mul(10));
    Ok(ListEntry {
        provider: ProviderKind::MyAnimeList,
        media_id: ProviderMediaId(media_id),
        mal_id: Some(media_id),
        status,
        progress_episodes: wire.num_episodes_watched.into_inner().unwrap_or(0),
        score_0_to_100,
        updated_at_epoch_s,
        title: String::new(),
    })
}

/// Parse an `/anime/{id}?fields=my_list_status` response down to the
/// authenticated user's current entry (status + watched count, and the
/// rewatch count where the read asked for it).
/// `my_list_status` is absent when the show isn't on the user's list →
/// `None`. An unrecognized status maps to `ListStatus::Watching` so a
/// malformed enum can't suppress the write-back reconcile. Powers the
/// monotonic + status-preserving guard.
pub(in crate::meta) fn parse_my_list_status_entry(body: &[u8]) -> Result<Option<CurrentEntry>> {
    #[derive(Deserialize)]
    struct Wire {
        // The anime itself: MAL leaves `my_list_status` out for a show
        // not on the list, so only the document around it tells that
        // answer from a reply that is no answer.
        #[serde(rename = "id")]
        _id: u64,
        #[serde(default)]
        my_list_status: Option<MyListStatus>,
    }
    #[derive(Deserialize)]
    struct MyListStatus {
        // MAL sends both on every list status; missing, the entry says
        // nothing of where the user is.
        status: crate::meta::present::Present<String>,
        num_episodes_watched: crate::meta::present::Present<u32>,
        #[serde(default)]
        is_rewatching: Option<bool>,
        #[serde(default)]
        num_times_rewatched: Option<u32>,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal anime my_list_status: {e}"),
    })?;
    Ok(wire.my_list_status.map(|s| {
        let rewatching = s.is_rewatching.unwrap_or(false);
        CurrentEntry {
            status: s
                .status
                .into_inner()
                .as_deref()
                .and_then(|st| ListStatus::from_mal(st, rewatching))
                .unwrap_or(ListStatus::Watching),
            progress_episodes: s.num_episodes_watched.into_inner().unwrap_or(0),
            times_rewatched: s.num_times_rewatched,
        }
    }))
}

/// Parse an `/anime/{id}?fields=my_list_status{…,num_times_rewatched},num_episodes`
/// response: the user's row as [`parse_my_list_status_entry`] reads it
/// (its rewatch count with it, which MAL returns only when asked), and the
/// anime's episode total, where MAL's `0` (length not announced) is
/// unknown.
pub(in crate::meta) fn parse_tracked_media(body: &[u8]) -> Result<TrackedMedia> {
    #[derive(Deserialize)]
    struct Wire {
        #[serde(default)]
        num_episodes: Option<u32>,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal anime num_episodes: {e}"),
    })?;
    Ok(TrackedMedia {
        entry: parse_my_list_status_entry(body)?,
        episodes: wire.num_episodes.filter(|n| *n > 0),
    })
}
