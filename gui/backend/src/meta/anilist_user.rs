//! AniList concrete [`UserListProvider`] implementation — OAuth +
//! `Viewer` profile + `MediaListCollection` user-list fetch.
//!
//! Reads only in PR #1. Write-back (`update_entry`, `delete_entry`)
//! lands in PR #4; the stubs here return [`AniError::Metadata`] until
//! then.
//!
//! AniList specifics worth keeping next to the code:
//!
//! - GraphQL endpoint: a single POST to `graphql.anilist.co`. Existing
//!   trending client lives at [`crate::meta::anilist`].
//! - OAuth: code-grant exchange returns a 1-year JWT; AniList does NOT
//!   issue refresh tokens, so [`AniListProvider::refresh`] is a hard
//!   `Err(AniError::Metadata)`. Disconnect = drop the token locally.
//! - PKCE: AniList ignores `code_challenge` / `code_challenge_method`.
//!   The trait still hands us a [`Pkce`] for symmetry with MAL; we
//!   never put it on the wire.
//! - User agent: matches the existing convention from
//!   [`crate::meta::anilist`] so any UA-based ratelimit treats both
//!   surfaces as the same client.
//! - Score scale: AniList's POINT_100 system is already 0..=100, which
//!   matches the unified scale; pass through with no conversion.

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Deserialize;

use crate::account::credentials::{
    ANILIST_API, ANILIST_AUTH_URL, ANILIST_CLIENT_ID, ANILIST_CLIENT_SECRET, ANILIST_REDIRECT_URI,
    ANILIST_TOKEN_URL,
};
use crate::account::pkce::Pkce;
use crate::account::provider::{
    EntryUpdate, ListEntry, ProviderKind, ProviderMediaId, Tokens, UserListProvider, UserProfile,
    UserStats,
};
use crate::account::status::ListStatus;
use crate::error::{AniError, Result};

/// User-agent advertised on every AniList request. Matches the format
/// used by [`crate::meta::anilist`] so AniList's Cloudflare layer
/// treats the two surfaces as the same client.
const ANILIST_USER_AGENT: &str = "ani-gui/0.1 (https://github.com/pucci/ani-gui)";

/// GraphQL: authenticated user's profile. Mirrors §4.1 of the
/// account-integration plan. `meanScore` on this surface is the
/// 0..=100 percentage AniList returns regardless of the user's
/// chosen scoring system — Codex P2 #3370087028. The read side
/// rescales to the trait's 0..=10 contract.
const VIEWER_GQL: &str = "query Viewer { \
        Viewer { \
            id name \
            avatar { large medium } \
            statistics { anime { count meanScore } } \
        } \
    }";

/// GraphQL: write-back mutation. Returns the resulting `MediaList`
/// row so the caller can echo the persisted state straight back to
/// the cache without a follow-up read. Score is requested via
/// `format: POINT_100` so the round-trip stays on the unified 0..=100
/// scale — see `MEDIA_LIST_GQL` for the rationale.
const SAVE_ENTRY_GQL: &str = "mutation Save( \
        $mediaId: Int!, $status: MediaListStatus, $progress: Int, \
        $scoreRaw: Int, $repeat: Int) { \
        SaveMediaListEntry(mediaId: $mediaId, status: $status, progress: $progress, \
            scoreRaw: $scoreRaw, repeat: $repeat) { \
            mediaId \
            status progress score(format: POINT_100) updatedAt repeat \
            media { idMal title { romaji english userPreferred } } \
        } \
    }";

/// GraphQL: resolve the entry's row id from `(mediaId, userId)`.
/// `DeleteMediaListEntry` takes the row id, not the media id — so
/// delete is a two-step query+mutation. The viewer id is fetched via
/// the same `me()` path the trait already uses.
const MEDIA_LIST_ROW_ID_GQL: &str = "query MediaList($mediaId: Int!, $userId: Int!) { \
        MediaList(mediaId: $mediaId, userId: $userId) { id } \
    }";

/// GraphQL: delete the resolved row.
const DELETE_ENTRY_GQL: &str = "mutation Delete($id: Int!) { \
        DeleteMediaListEntry(id: $id) { deleted } \
    }";

/// GraphQL: paginated full user list. `perChunk: 500` matches the
/// upper bound AniList advertises per request — for a 312-entry
/// user (the median in our test fixtures) the loop terminates after
/// one round-trip. The trait surface accepts the pagination cost so
/// every concrete provider hides chunk semantics from rail callers.
/// `score(format: POINT_100)` pins the returned value to the unified
/// 0..=100 scale regardless of the user's AniList scoring preference.
/// Without the format arg the user's preferred system (POINT_10,
/// POINT_5_DECIMAL, etc.) leaks through, and an 8/10 silently
/// becomes 8/100 in the cache.
const MEDIA_LIST_GQL: &str = "query MediaList($userId: Int!, $chunk: Int!) { \
        MediaListCollection(userId: $userId, type: ANIME, chunk: $chunk, perChunk: 500) { \
            hasNextChunk \
            lists { \
                status \
                entries { \
                    mediaId \
                    status progress score(format: POINT_100) updatedAt repeat \
                    media { idMal title { romaji english userPreferred } } \
                } \
            } \
        } \
    }";

/// AniList implementation of [`UserListProvider`].
///
/// Two endpoint overrides — `api_base` for the GraphQL endpoint and
/// `token_base` for the OAuth token-exchange endpoint — let tests
/// point at wiremock while production hits the real `anilist.co`.
pub struct AniListProvider {
    client: reqwest::Client,
    /// Override for the GraphQL endpoint. `None` → production
    /// [`ANILIST_API`]. Tests pass a wiremock URI.
    api_base: Option<String>,
    /// Override for the OAuth token endpoint. `None` → production
    /// [`ANILIST_TOKEN_URL`]. Tests pass a wiremock URI.
    token_base: Option<String>,
}

impl AniListProvider {
    /// Build a provider that hits production AniList endpoints.
    #[must_use]
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            api_base: None,
            token_base: None,
        }
    }

    /// Build a provider with wiremock-style endpoint overrides — the
    /// test harness mounts mock responses on these URIs.
    #[must_use]
    pub fn with_bases(client: reqwest::Client, api_base: String, token_base: String) -> Self {
        Self {
            client,
            api_base: Some(api_base),
            token_base: Some(token_base),
        }
    }

    fn api_url(&self) -> &str {
        self.api_base.as_deref().unwrap_or(ANILIST_API)
    }

    fn token_url(&self) -> &str {
        self.token_base.as_deref().unwrap_or(ANILIST_TOKEN_URL)
    }

    /// Shared POST helper for the GraphQL endpoint. Handles the
    /// Bearer header + user-agent + the AniList-specific status
    /// mapping: 401 → [`AniError::InvalidToken`] (revoked / expired
    /// token; route layer surfaces "Sign in again"), any other
    /// non-2xx → [`AniError::Upstream`].
    async fn post_graphql(
        &self,
        tokens: &Tokens,
        body: &serde_json::Value,
    ) -> Result<bytes::Bytes> {
        let resp = self
            .client
            .post(self.api_url())
            .header("user-agent", ANILIST_USER_AGENT)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .bearer_auth(&tokens.access_token)
            .json(body)
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AniError::InvalidToken);
        }
        if !status.is_success() {
            return Err(AniError::Upstream {
                status: status.as_u16(),
            });
        }
        resp.bytes().await.map_err(|_| AniError::Network)
    }
}

#[async_trait]
impl UserListProvider for AniListProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::AniList
    }

    fn auth_url(&self, _pkce: &Pkce, state: &str) -> Result<String> {
        // AniList ignores PKCE entirely — we deliberately do not emit
        // `code_challenge` / `code_challenge_method` here. The trait
        // still hands us a `Pkce` so the MAL impl can mirror the
        // signature, but on the wire we'd only confuse AniList's
        // parser. See module doc-comment.
        //
        // `url::Url::parse_with_params` percent-encodes the values
        // per `application/x-www-form-urlencoded` — same encoding
        // AniList's authorize endpoint expects.
        let params = [
            ("client_id", ANILIST_CLIENT_ID),
            ("redirect_uri", ANILIST_REDIRECT_URI),
            ("response_type", "code"),
            ("state", state),
        ];
        Ok(url::Url::parse_with_params(ANILIST_AUTH_URL, &params)
            .map(String::from)
            .unwrap_or_default())
    }

    async fn exchange_code(&self, code: &str, _pkce: &Pkce) -> Result<Tokens> {
        // AniList's token endpoint accepts the OAuth code-grant body
        // as JSON. Their docs show form-urlencoded too; we use JSON
        // here because the rest of the AniList surface is JSON.
        // The `_pkce` parameter is ignored — AniList doesn't read it.
        let body = serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": ANILIST_CLIENT_ID,
            "client_secret": ANILIST_CLIENT_SECRET,
            "redirect_uri": ANILIST_REDIRECT_URI,
            "code": code,
        });
        let resp = self
            .client
            .post(self.token_url())
            .header("user-agent", ANILIST_USER_AGENT)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|_| AniError::Network)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(AniError::Upstream {
                status: status.as_u16(),
            });
        }
        let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
        parse_token_response(&bytes)
    }

    /// AniList does not issue refresh tokens. Their 1-year JWT has
    /// no refresh flow and no revocation endpoint — disconnect on
    /// our side is "drop the token locally" and re-prompt the user.
    /// Returning [`AniError::Metadata`] keeps this distinct from
    /// transient Network / Upstream failures so the route layer can
    /// surface a "this provider has no refresh flow" message rather
    /// than retrying.
    async fn refresh(&self, _refresh_token: &str) -> Result<Tokens> {
        Err(AniError::Metadata)
    }

    async fn me(&self, tokens: &Tokens) -> Result<UserProfile> {
        let body = serde_json::json!({ "query": VIEWER_GQL });
        let bytes = self.post_graphql(tokens, &body).await?;
        parse_viewer_response(&bytes)
    }

    async fn list_all(&self, tokens: &Tokens) -> Result<Vec<ListEntry>> {
        // AniList's MediaListCollection wants `$userId` as a
        // non-negotiable filter; we resolve it via `me()` so the
        // trait stays minimal (one call site doesn't need to pass
        // the user id through to every list fetch).
        let me = self.me(tokens).await?;
        let user_id: i64 = me.user_id.parse().map_err(|_| AniError::ParseFailed {
            detail: format!("anilist viewer id not numeric: {}", me.user_id),
        })?;

        let mut out: Vec<ListEntry> = Vec::new();
        let mut chunk: i64 = 1;
        loop {
            let body = serde_json::json!({
                "query": MEDIA_LIST_GQL,
                "variables": { "userId": user_id, "chunk": chunk },
            });
            let bytes = self.post_graphql(tokens, &body).await?;
            let page = parse_media_list_page(&bytes)?;
            for entry in page.entries {
                out.push(entry);
            }
            if !page.has_next_chunk {
                break;
            }
            chunk += 1;
        }
        Ok(out)
    }

    async fn update_entry(
        &self,
        tokens: &Tokens,
        id: ProviderMediaId,
        update: EntryUpdate,
    ) -> Result<ListEntry> {
        // Build the variables map. AniList's GraphQL ignores absent
        // arg keys (leaves the field unchanged); `null` is treated as
        // an explicit clear. We only emit fields the caller asked to
        // change so partial updates don't accidentally null
        // siblings.
        let mut vars = serde_json::Map::new();
        vars.insert("mediaId".into(), serde_json::json!(id.0));
        if let Some(status) = update.status {
            vars.insert("status".into(), serde_json::json!(status.to_anilist()));
        }
        if let Some(progress) = update.progress_episodes {
            vars.insert("progress".into(), serde_json::json!(progress));
        }
        if let Some(score) = update.score_0_to_100 {
            // AniList's `scoreRaw` arg is the POINT_100 integer; the
            // unified 0..=100 scale lines up 1:1.
            vars.insert("scoreRaw".into(), serde_json::json!(score));
        }
        if let Some(repeat) = update.repeat_count {
            vars.insert("repeat".into(), serde_json::json!(repeat));
        }
        let body = serde_json::json!({
            "query": SAVE_ENTRY_GQL,
            "variables": vars,
        });
        let bytes = self.post_graphql(tokens, &body).await?;
        parse_save_entry_response(&bytes)
    }

    async fn delete_entry(&self, tokens: &Tokens, id: ProviderMediaId) -> Result<()> {
        // Two-step: AniList's `DeleteMediaListEntry` mutation takes
        // the MediaList row id, not the media id. Resolve the row
        // id first by looking up `(mediaId, userId)`, then fire the
        // delete. Three round-trips total (me + row-id lookup +
        // delete); each hits a 401 short-circuit so a revoked token
        // surfaces immediately as `InvalidToken` instead of after a
        // partial success.
        let me = self.me(tokens).await?;
        let user_id: i64 = me.user_id.parse().map_err(|_| AniError::ParseFailed {
            detail: format!("anilist viewer id not numeric: {}", me.user_id),
        })?;
        let lookup_body = serde_json::json!({
            "query": MEDIA_LIST_ROW_ID_GQL,
            "variables": { "mediaId": id.0, "userId": user_id },
        });
        let lookup_bytes = self.post_graphql(tokens, &lookup_body).await?;
        let row_id = parse_media_list_row_id(&lookup_bytes)?;
        let delete_body = serde_json::json!({
            "query": DELETE_ENTRY_GQL,
            "variables": { "id": row_id },
        });
        let bytes = self.post_graphql(tokens, &delete_body).await?;
        // AniList answers with `{ deleted: Boolean }`; `false` is its
        // documented failure signal. Surface it as an error so a future
        // retry/cache layer doesn't drop the local row + stop retrying
        // for an entry the provider never removed (Codex P2
        // #3381101376).
        if parse_delete_result(&bytes)? {
            Ok(())
        } else {
            Err(AniError::Metadata)
        }
    }
}

/// Pure parser for the `Viewer` GraphQL response. AniList ids are
/// numeric on the wire; the trait carries `user_id` as a `String`
/// so MAL's `@me` id later fits the same shape.
///
/// Avatar URL preference: `large` first, then `medium`. The trait
/// surface keeps a single `avatar_url` rather than a bag because
/// the chip + popover only render one size.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the response isn't the
/// documented `{ data: { Viewer: { … } } }` envelope.
fn parse_viewer_response(body: &[u8]) -> Result<UserProfile> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Viewer")]
        viewer: Viewer,
    }
    #[derive(Deserialize)]
    struct Viewer {
        id: u64,
        name: String,
        avatar: Option<Avatar>,
        statistics: Option<Statistics>,
    }
    #[derive(Deserialize)]
    struct Avatar {
        large: Option<String>,
        medium: Option<String>,
    }
    #[derive(Deserialize)]
    struct Statistics {
        anime: Option<AnimeStats>,
    }
    #[derive(Deserialize)]
    struct AnimeStats {
        count: u32,
        #[serde(rename = "meanScore")]
        mean_score: Option<f32>,
    }
    let wire: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist viewer response: {e}"),
    })?;
    let avatar_url = wire.data.viewer.avatar.and_then(|a| a.large.or(a.medium));
    let stats = wire.data.viewer.statistics.and_then(|s| s.anime).map(|a| {
        UserStats {
            anime_count: a.count,
            // AniList returns meanScore in 0..=100 (percentage points)
            // regardless of the user's chosen scoring system —
            // POINT_100, POINT_10, POINT_5, etc. all surface here as a
            // percentage. The trait's contract is 0..=10, so divide.
            // Codex P2 #3370087028: prior pass-through showed 65.5/10
            // for a 100-point user with a 65.5% mean.
            mean_score_0_to_10: a.mean_score.map(|s| s / 10.0),
        }
    });
    Ok(UserProfile {
        provider: ProviderKind::AniList,
        user_id: wire.data.viewer.id.to_string(),
        username: wire.data.viewer.name,
        avatar_url,
        stats,
    })
}

/// One decoded MediaListCollection page — the chunk's entries
/// (already translated into the trait shape) plus the
/// has-next-chunk flag the paginator loops on.
struct MediaListPage {
    entries: Vec<ListEntry>,
    has_next_chunk: bool,
}

/// Pure parser for the MediaListCollection chunk response. Drops
/// entries whose status doesn't translate (rare draft / half-saved
/// AniList state — surfacing them as a hard error would break the
/// rail for the whole list).
///
/// Title fallback: `userPreferred` → `romaji` → `english` →
/// `"(untitled)"` (the rail renders this via Kitsu metadata once the
/// `mal_id` bridge resolves, so a literal fallback is acceptable).
///
/// Score: AniList's POINT_100 system writes the score as a float
/// 0.0..=100.0 on the wire (the 0..=10 surface comes from other
/// scoring systems we ignore in v1). 0.0 surfaces as `None` per the
/// "unrated" convention; non-zero clamps to u8 via `.min(100)` to
/// guard against arithmetic underflow if AniList ever bumps the
/// scale ceiling.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the response isn't the
/// documented `{ data: { MediaListCollection: { … } } }` envelope.
fn parse_media_list_page(body: &[u8]) -> Result<MediaListPage> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "MediaListCollection")]
        media_list_collection: Collection,
    }
    #[derive(Deserialize)]
    struct Collection {
        #[serde(rename = "hasNextChunk")]
        has_next_chunk: bool,
        #[serde(default)]
        lists: Vec<ListBucket>,
    }
    #[derive(Deserialize)]
    struct ListBucket {
        #[serde(default)]
        entries: Vec<RawEntry>,
    }
    #[derive(Deserialize)]
    struct RawEntry {
        #[serde(rename = "mediaId")]
        media_id: u32,
        status: String,
        progress: u32,
        score: f32,
        #[serde(rename = "updatedAt")]
        updated_at: i64,
        media: RawMedia,
    }
    #[derive(Deserialize)]
    struct RawMedia {
        #[serde(rename = "idMal")]
        id_mal: Option<u32>,
        title: RawTitle,
    }
    #[derive(Deserialize)]
    struct RawTitle {
        romaji: Option<String>,
        english: Option<String>,
        #[serde(rename = "userPreferred")]
        user_preferred: Option<String>,
    }

    let wire: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist media list response: {e}"),
    })?;
    let collection = wire.data.media_list_collection;

    let mut entries = Vec::new();
    for bucket in collection.lists {
        for raw in bucket.entries {
            let Some(status) = ListStatus::from_anilist(&raw.status) else {
                // Skip — the unified enum has no slot for this
                // value; surfacing it as a hard error would break
                // the rail renderer for the whole list.
                continue;
            };
            let score_0_to_100 = if raw.score == 0.0 {
                None
            } else {
                Some((raw.score as u32).min(100) as u8)
            };
            let title = raw
                .media
                .title
                .user_preferred
                .or(raw.media.title.romaji)
                .or(raw.media.title.english)
                .unwrap_or_else(|| "(untitled)".to_string());
            entries.push(ListEntry {
                provider: ProviderKind::AniList,
                media_id: ProviderMediaId(raw.media_id),
                mal_id: raw.media.id_mal,
                status,
                progress_episodes: raw.progress,
                score_0_to_100,
                updated_at_epoch_s: raw.updated_at,
                title,
            });
        }
    }
    Ok(MediaListPage {
        entries,
        has_next_chunk: collection.has_next_chunk,
    })
}

/// Pure parser for the `SaveMediaListEntry` mutation response. Shape
/// mirrors a single `entries[]` row from the list query so it
/// re-uses the same mapping logic (status translation, score
/// rescale, title pick).
///
/// # Errors
/// [`AniError::ParseFailed`] when the response isn't the documented
/// `{ data: { SaveMediaListEntry: { … } } }` envelope, or when the
/// row carries a status the unified enum doesn't recognise.
fn parse_save_entry_response(body: &[u8]) -> Result<ListEntry> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "SaveMediaListEntry")]
        save_media_list_entry: RawEntry,
    }
    #[derive(Deserialize)]
    struct RawEntry {
        #[serde(rename = "mediaId")]
        media_id: u32,
        status: String,
        progress: u32,
        score: f32,
        #[serde(rename = "updatedAt")]
        updated_at: i64,
        media: RawMedia,
    }
    #[derive(Deserialize)]
    struct RawMedia {
        #[serde(rename = "idMal")]
        id_mal: Option<u32>,
        title: RawTitle,
    }
    #[derive(Deserialize)]
    struct RawTitle {
        romaji: Option<String>,
        english: Option<String>,
        #[serde(rename = "userPreferred")]
        user_preferred: Option<String>,
    }
    let wire: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist save entry response: {e}"),
    })?;
    let raw = wire.data.save_media_list_entry;
    let status = ListStatus::from_anilist(&raw.status).ok_or_else(|| AniError::ParseFailed {
        detail: format!("anilist save entry unknown status: {}", raw.status),
    })?;
    let score_0_to_100 = if raw.score == 0.0 {
        None
    } else {
        Some((raw.score as u32).min(100) as u8)
    };
    let title = raw
        .media
        .title
        .user_preferred
        .or(raw.media.title.romaji)
        .or(raw.media.title.english)
        .unwrap_or_else(|| "(untitled)".to_string());
    Ok(ListEntry {
        provider: ProviderKind::AniList,
        media_id: ProviderMediaId(raw.media_id),
        mal_id: raw.media.id_mal,
        status,
        progress_episodes: raw.progress,
        score_0_to_100,
        updated_at_epoch_s: raw.updated_at,
        title,
    })
}

/// Pure parser for the `DeleteMediaListEntry` mutation response.
/// Returns the `deleted` flag; the caller treats `false` as a failed
/// delete (Codex P2 #3381101376).
///
/// # Errors
/// [`AniError::ParseFailed`] when the envelope isn't the documented
/// `{ data: { DeleteMediaListEntry: { deleted } } }` shape.
fn parse_delete_result(body: &[u8]) -> Result<bool> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "DeleteMediaListEntry")]
        delete_media_list_entry: Deleted,
    }
    #[derive(Deserialize)]
    struct Deleted {
        deleted: bool,
    }
    let wire: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist delete response: {e}"),
    })?;
    Ok(wire.data.delete_media_list_entry.deleted)
}

/// Pure parser for the `MediaList(mediaId, userId)` row-id lookup.
/// Returns the row id so `delete_entry` can call
/// `DeleteMediaListEntry(id)` against it.
///
/// # Errors
/// [`AniError::ParseFailed`] when the envelope is wrong; the AniList
/// API returns a GraphQL error (not a 404) when the entry doesn't
/// exist, so a missing `MediaList` field is surfaced as a parse
/// failure rather than a sentinel "already deleted" success.
fn parse_media_list_row_id(body: &[u8]) -> Result<i64> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "MediaList")]
        media_list: Row,
    }
    #[derive(Deserialize)]
    struct Row {
        id: i64,
    }
    let wire: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist media list row-id response: {e}"),
    })?;
    Ok(wire.data.media_list.id)
}

/// Sentinel expiry when neither `expires_in` nor a decodable JWT `exp`
/// claim is available. AniList's tokens are documented as essentially
/// non-expiring; a 1-year window matches their nominal `expires_in`
/// when they DO send it, and keeps the renderer's expiry check
/// (`accountStore.hydrate` → `isExpired(payload)`) honest. A real
/// auth failure surfaces later as the `me()` 401 → `expired` state
/// regardless of this number — this just keeps the Connect flow from
/// rejecting a perfectly good token on the spot.
const ANILIST_FALLBACK_EXPIRY_S: i64 = 31_536_000;

/// Pure parser for AniList's OAuth token-exchange response.
///
/// Shape: `{token_type, [expires_in], access_token, refresh_token}`.
/// AniList in practice always returns `refresh_token: null` — the
/// trait carries it as `Option<String>` so MAL's real refresh tokens
/// fit the same struct.
///
/// `expires_in` is documented but inconsistently present — Codex P1
/// #3371176290. When absent we try to decode the JWT `exp` claim from
/// the access_token (no signature verification — we only trust the
/// issuer-stated wall-clock window for the local hydrate gate); if
/// that fails too, we fall back to [`ANILIST_FALLBACK_EXPIRY_S`].
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the response isn't the
/// documented shape.
fn parse_token_response(body: &[u8]) -> Result<Tokens> {
    #[derive(Deserialize)]
    struct Wire {
        access_token: String,
        #[serde(default)]
        refresh_token: Option<String>,
        #[serde(default)]
        expires_in: Option<i64>,
    }
    let wire: Wire = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist token response: {e}"),
    })?;
    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let expires_at_epoch_s = resolve_token_expiry(wire.expires_in, &wire.access_token, now_s);
    Ok(Tokens {
        access_token: wire.access_token,
        refresh_token: wire.refresh_token,
        expires_at_epoch_s,
    })
}

/// Resolve an absolute expiry epoch from three sources, in order of
/// trust:
///
///   1. The wire's `expires_in` (relative seconds → absolute).
///   2. The JWT `exp` claim decoded from the access token (absolute
///      epoch). We do NOT verify the signature — we only believe the
///      token's own stated expiry for the local hydrate gate; the
///      provider validates the signature on every authenticated call.
///   3. [`ANILIST_FALLBACK_EXPIRY_S`] from now — last-resort sentinel
///      so an exchange that succeeded server-side doesn't get rejected
///      client-side just because we can't tell when it'll expire.
///
/// Extracted as a pure helper so unit tests can pin every branch
/// without a wiremock fixture.
fn resolve_token_expiry(expires_in: Option<i64>, access_token: &str, now_s: i64) -> i64 {
    if let Some(s) = expires_in {
        return now_s.saturating_add(s);
    }
    if let Some(exp) = jwt_exp_claim(access_token) {
        return exp;
    }
    now_s.saturating_add(ANILIST_FALLBACK_EXPIRY_S)
}

/// Decode the `exp` claim from a JWT-shaped access token without
/// verifying the signature.
///
/// Returns `None` on any failure (non-JWT shape, base64url decode
/// error, non-JSON payload, missing `exp`, non-integer `exp`). The
/// signature is never validated here — that's the provider's
/// responsibility on every authenticated call; we only trust the
/// issuer-stated expiry for our local hydrate gate.
fn jwt_exp_claim(access_token: &str) -> Option<i64> {
    use base64::Engine;
    let mut parts = access_token.split('.');
    let _header = parts.next()?;
    let payload_b64 = parts.next()?;
    // A real JWT has exactly three segments; opaque strings often have
    // zero or one. Tolerate trailing junk (parts.next() may yield
    // anything) but require at least the header + payload segments to
    // have been present.
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .ok()?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    json.get("exp")?.as_i64()
}

#[cfg(test)]
#[path = "anilist_user_test.rs"]
mod tests;
