//! MAL's list-status replies and the ISO-8601 reader their timestamps
//! go through; split from [`super`] so each file stays inside the CRAP
//! gate's per-file bar.

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
        #[serde(default)]
        num_episodes_watched: Option<u32>,
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
        progress_episodes: wire.num_episodes_watched.unwrap_or(0),
        score_0_to_100,
        updated_at_epoch_s,
        title: String::new(),
    })
}

/// Parse an `/anime/{id}?fields=my_list_status` response down to the
/// authenticated user's current entry (status + watched count).
/// `my_list_status` is absent when the show isn't on the user's list →
/// `None`. An unrecognized status maps to `ListStatus::Watching` so a
/// malformed enum can't suppress the write-back reconcile. Powers the
/// monotonic + status-preserving guard (Codex P1 #3386909281, P2
/// #3387319861 / #3387383171).
pub(in crate::meta) fn parse_my_list_status_entry(body: &[u8]) -> Result<Option<CurrentEntry>> {
    #[derive(Deserialize)]
    struct Wire {
        #[serde(default)]
        my_list_status: Option<MyListStatus>,
    }
    #[derive(Deserialize)]
    struct MyListStatus {
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        num_episodes_watched: Option<u32>,
        #[serde(default)]
        is_rewatching: Option<bool>,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal anime my_list_status: {e}"),
    })?;
    Ok(wire.my_list_status.map(|s| CurrentEntry {
        status: s
            .status
            .as_deref()
            .and_then(|st| ListStatus::from_mal(st, s.is_rewatching.unwrap_or(false)))
            .unwrap_or(ListStatus::Watching),
        progress_episodes: s.num_episodes_watched.unwrap_or(0),
    }))
}

/// Minimal RFC 3339 / ISO 8601 parser. MAL always emits the canonical
/// `YYYY-MM-DDTHH:MM:SS±HH:MM` (or trailing `Z`) shape — we extract
/// the date + time numerically and normalise the trailing offset to
/// UTC. The offset used to be ignored — safe only while these values
/// were compared with each other, since every row carried the same
/// skew — but the Watch Later rail now sorts them against AniList's
/// true Unix seconds.
///
/// Returns 0 for unparseable input so a malformed row doesn't fail
/// the whole list page.
pub(in crate::meta) fn parse_iso8601_to_epoch(s: &str) -> i64 {
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return 0;
    }
    let parse_u = |start: usize, end: usize| -> Option<i64> {
        std::str::from_utf8(&bytes[start..end]).ok()?.parse().ok()
    };
    let Some(y) = parse_u(0, 4) else { return 0 };
    let Some(m) = parse_u(5, 7) else { return 0 };
    let Some(d) = parse_u(8, 10) else { return 0 };
    let Some(hh) = parse_u(11, 13) else { return 0 };
    let Some(mm) = parse_u(14, 16) else { return 0 };
    let Some(ss) = parse_u(17, 19) else { return 0 };
    // Howard Hinnant's days_from_civil algorithm — exact for all
    // years in the proleptic Gregorian calendar.
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m_adj = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m_adj + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let local = days * 86400 + hh * 3600 + mm * 60 + ss;
    local - utc_offset_secs(bytes)
}

/// Seconds to subtract to turn the wall clock into UTC. `Z`, a
/// missing suffix, or anything malformed all mean "already UTC" —
/// a row whose offset cannot be read is better placed by its wall
/// clock than dropped from the list.
fn utc_offset_secs(bytes: &[u8]) -> i64 {
    let sign = match bytes.get(19) {
        Some(b'+') => 1,
        Some(b'-') => -1,
        _ => return 0,
    };
    if bytes.len() < 25 {
        return 0;
    }
    let num = |start: usize, end: usize| -> Option<i64> {
        std::str::from_utf8(&bytes[start..end]).ok()?.parse().ok()
    };
    let (Some(oh), Some(om)) = (num(20, 22), num(23, 25)) else {
        return 0;
    };
    sign * (oh * 3600 + om * 60)
}
