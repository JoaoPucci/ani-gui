//! Wire-shape parsers for the MAL provider. Extracted so the parse
//! density (struct-per-endpoint definitions, paginator details, the
//! inline ISO-8601 reader) doesn't pile onto `mal_user.rs`'s CRAP
//! score — the trait-impl file stays focused on network plumbing +
//! the refresh coalesce, parsers live here and are tested through
//! the wiremock surface in `mal_user_test.rs`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::account::provider::{
    ListEntry, ProviderKind, ProviderMediaId, Tokens, UserProfile, UserStats,
};
use crate::account::status::ListStatus;
use crate::error::{AniError, Result};

/// Parse MAL's OAuth token-exchange response into [`Tokens`]. Both
/// `exchange_code` and `refresh` use this — the wire shape is
/// identical between the initial grant and refresh responses.
pub(super) fn parse_token_response(body: &[u8]) -> Result<Tokens> {
    #[derive(Deserialize)]
    struct Wire {
        access_token: String,
        #[serde(default)]
        refresh_token: Option<String>,
        #[serde(default)]
        expires_in: Option<i64>,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal token response: {e}"),
    })?;
    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // MAL always sends expires_in; fall back to 1 hour (their stated
    // ceiling) so a missing field doesn't cause an immediate-expiry
    // disconnect on the next handler call.
    let expires_at_epoch_s = now_s + wire.expires_in.unwrap_or(3600);
    Ok(Tokens {
        access_token: wire.access_token,
        refresh_token: wire.refresh_token,
        expires_at_epoch_s,
    })
}

/// Parse MAL's `/v2/users/@me?fields=anime_statistics` response into
/// the unified [`UserProfile`]. Mean score stays on the unified
/// 0..=10 scale — no rescaling needed for MAL.
pub(super) fn parse_viewer_response(body: &[u8]) -> Result<UserProfile> {
    #[derive(Deserialize)]
    struct Wire {
        id: u64,
        name: String,
        #[serde(default)]
        picture: Option<String>,
        #[serde(default)]
        anime_statistics: Option<AnimeStats>,
    }
    #[derive(Deserialize)]
    struct AnimeStats {
        #[serde(default)]
        num_items: Option<u32>,
        #[serde(default)]
        num_items_completed: Option<u32>,
        #[serde(default)]
        num_items_watching: Option<u32>,
        #[serde(default)]
        num_items_on_hold: Option<u32>,
        #[serde(default)]
        num_items_dropped: Option<u32>,
        #[serde(default)]
        num_items_plan_to_watch: Option<u32>,
        #[serde(default)]
        mean_score: Option<f32>,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal viewer response: {e}"),
    })?;
    let stats = wire.anime_statistics.map(|a| {
        let count = a.num_items.unwrap_or_else(|| {
            a.num_items_watching.unwrap_or(0)
                + a.num_items_completed.unwrap_or(0)
                + a.num_items_on_hold.unwrap_or(0)
                + a.num_items_dropped.unwrap_or(0)
                + a.num_items_plan_to_watch.unwrap_or(0)
        });
        UserStats {
            anime_count: count,
            mean_score_0_to_10: a.mean_score.filter(|s| *s > 0.0),
        }
    });
    Ok(UserProfile {
        provider: ProviderKind::MyAnimeList,
        user_id: wire.id.to_string(),
        username: wire.name,
        avatar_url: wire.picture,
        stats,
    })
}

pub(super) struct MalListPage {
    pub entries: Vec<ListEntry>,
    pub next_url: Option<String>,
}

pub(super) fn parse_list_page(body: &[u8]) -> Result<MalListPage> {
    #[derive(Deserialize)]
    struct Wire {
        data: Vec<WireRow>,
        #[serde(default)]
        paging: Option<Paging>,
    }
    #[derive(Deserialize)]
    struct Paging {
        #[serde(default)]
        next: Option<String>,
    }
    #[derive(Deserialize)]
    struct WireRow {
        node: WireNode,
        list_status: WireListStatus,
    }
    #[derive(Deserialize)]
    struct WireNode {
        id: u32,
        #[serde(default)]
        title: String,
    }
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
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("mal list_all page: {e}"),
    })?;
    let mut entries = Vec::with_capacity(wire.data.len());
    for row in wire.data {
        let Some(status) = ListStatus::from_mal(
            &row.list_status.status,
            row.list_status.is_rewatching.unwrap_or(false),
        ) else {
            // Unknown status — log + skip rather than fail the whole
            // page (mirrors AniList's tolerance for unrecognised
            // enum values).
            continue;
        };
        let updated_at_epoch_s = row
            .list_status
            .updated_at
            .as_deref()
            .map_or(0, parse_iso8601_to_epoch);
        // MAL scores are 0..=10 integer; the cache stores 0..=100.
        // 0 means "unrated" — drop it so the popover doesn't render
        // "0/10" for users who haven't scored anything.
        let score_0_to_100 = row
            .list_status
            .score
            .filter(|s| *s > 0)
            .map(|s| s.saturating_mul(10));
        entries.push(ListEntry {
            provider: ProviderKind::MyAnimeList,
            media_id: ProviderMediaId(row.node.id),
            mal_id: Some(row.node.id),
            status,
            progress_episodes: row.list_status.num_episodes_watched.unwrap_or(0),
            score_0_to_100,
            updated_at_epoch_s,
            title: row.node.title,
        });
    }
    Ok(MalListPage {
        entries,
        next_url: wire.paging.and_then(|p| p.next),
    })
}

/// Parse a bare `my_list_status` response (PATCH return body). MAL
/// returns just the updated `list_status` here, no anime metadata —
/// the caller already knows which anime they PATCHed. The trait
/// returns `ListEntry`, so the caller supplies the media id; title
/// is left empty (the cache write-through merges with the row from
/// `list_all` if present).
pub(super) fn parse_list_status_response(body: &[u8], media_id: u32) -> Result<ListEntry> {
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

/// Minimal RFC 3339 / ISO 8601 parser. MAL always emits the canonical
/// `YYYY-MM-DDTHH:MM:SS±HH:MM` (or trailing `Z`) shape — we extract
/// the date + time numerically and ignore the trailing offset (the
/// epoch the cache stores is treated as UTC; ordering across rows
/// stays correct because every row is from the same user).
///
/// Returns 0 for unparseable input so a malformed row doesn't fail
/// the whole list page.
pub(super) fn parse_iso8601_to_epoch(s: &str) -> i64 {
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
    days * 86400 + hh * 3600 + mm * 60 + ss
}
