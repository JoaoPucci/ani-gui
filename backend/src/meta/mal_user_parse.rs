//! Wire-shape parsers for the MAL provider. Extracted so the parse
//! density (struct-per-endpoint definitions, paginator details, the
//! inline ISO-8601 reader) doesn't pile onto `mal_user.rs`'s CRAP
//! score — the trait-impl file stays focused on network plumbing +
//! the refresh coalesce, parsers live here and are tested through
//! the wiremock surface in `mal_user_test.rs`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::account::provider::{
    CurrentEntry, ListEntry, ProviderKind, ProviderMediaId, Tokens, UserProfile, UserStats,
};
use crate::account::status::ListStatus;
use crate::error::{AniError, Result};

#[path = "mal_user_parse_status.rs"]
mod status;
pub(super) use status::{
    parse_iso8601_to_epoch, parse_list_status_response, parse_my_list_status_entry,
};

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
