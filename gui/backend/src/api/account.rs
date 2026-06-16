//! Account integration routes.
//!
//! Extracted to its own submodule (mirroring `api::syncplay`) to keep
//! `api::mod`'s ccn in check — adding the eight new handlers inline
//! would push that file's CRAP score over the ratchet ceiling.
//!
//! Routes mounted by [`router`]:
//!
//! | Method   | Path                                | Purpose |
//! |----------|-------------------------------------|---------|
//! | POST     | /api/account/auth-url/:provider     | Build OAuth authorize URL + return PKCE pair |
//! | POST     | /api/account/exchange-code/:provider | Exchange code for tokens (renderer persists via safeStorage) |
//! | POST     | /api/account/me/:provider           | Fetch profile (Authorization: Bearer header) |
//! | POST     | /api/account/list/:provider         | Fetch + cache list (Authorization: Bearer header) |
//! | GET      | /api/account/list/:provider/cached  | Read cached list (`?user_id=…`) |
//! | DELETE   | /api/account/list/:provider/cache   | Drop cache rows (`?user_id=…`) |
//!
//! Statelessness: the backend holds NO token state. Every authenticated
//! call carries the bearer in `Authorization: Bearer …`. The renderer
//! reads tokens from Electron's `safeStorage` on every call. See
//! `commands::account` doc for the full lifecycle.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};

use crate::account::provider::{ListEntry, ProviderKind, Tokens, UserProfile};
use crate::app::AppState;
use crate::commands::account;
use crate::error::AniError;

/// Mount the account routes onto the given app state. Called from
/// [`crate::api::build_api_router`].
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/account/auth-url/:provider", post(post_auth_url))
        .route(
            "/api/account/exchange-code/:provider",
            post(post_exchange_code),
        )
        .route("/api/account/refresh/:provider", post(post_refresh))
        .route("/api/account/me/:provider", post(post_me))
        .route("/api/account/update/:provider", post(post_update))
        .route("/api/account/set/:provider", post(post_set))
        .route(
            "/api/account/entry/:provider",
            get(get_entry).delete(delete_entry),
        )
        .route("/api/account/list/:provider", post(post_list))
        .route("/api/account/list/:provider/cached", get(get_cached_list))
        .route(
            "/api/account/list/:provider/cache",
            delete(delete_list_cache),
        )
        .route(
            "/api/account/list/:provider/cache/all",
            delete(delete_list_cache_all),
        )
}

// Wire types (request/response structs + conversions) live in the
// sibling `account_wire` module so this handler file stays under the
// CRAP ratchet ceiling — see account_wire.rs.
#[path = "account_wire.rs"]
mod account_wire;
use account_wire::{
    AuthUrlRequest, AuthUrlResponse, DisconnectFallbackQuery, EntryQuery, EntryView,
    ExchangeCodeRequest, RefreshRequest, SetEntryRequest, TokensResponse, UpdateProgressRequest,
};

/// When the disconnect-path `me()` call fails, decide whether to fall
/// through to the renderer-supplied identity (still gated by the
/// internal secret). Codex P2 #3370011855 opened the path for
/// `InvalidToken`; Codex P2 #3370096596 extends it to `Network` and
/// `Upstream` so an offline disconnect can still clear the local cache
/// rows the docs/PRIVACY.md promise to drop. Other variants (Io, etc.)
/// still propagate as before — they indicate the backend itself is
/// broken, not that the upstream identity can't be reached.
fn me_failure_allows_renderer_fallback(e: &AniError) -> bool {
    matches!(
        e,
        AniError::InvalidToken | AniError::Network | AniError::Upstream { .. }
    )
}

/// Resolve the cache-owner user_id for an endpoint that operates on
/// per-user cache rows. Tries the bearer-derived `me()` identity
/// first (the only one upstream can vouch for), and on offline /
/// 401 / 5xx falls back to the renderer-supplied id gated by the
/// internal secret. Used by both the cached-read path (Codex P2
/// #3372942241) and the disconnect-delete path (Codex P2 #3370011855
/// + #3370096596) so a single helper enforces the security gate.
async fn resolve_owner_user_id(
    state: &Arc<AppState>,
    kind: ProviderKind,
    tokens: &Tokens,
    headers: &HeaderMap,
    fallback: Option<String>,
) -> Result<String, AniError> {
    match account::me(state, kind, tokens).await {
        Ok(profile) => Ok(profile.user_id),
        Err(e) if me_failure_allows_renderer_fallback(&e) => {
            state.internal_secret.validate_header(headers)?;
            fallback.ok_or(e)
        }
        Err(e) => Err(e),
    }
}

// — Handlers — — — — — — — — — — — — — — — — — — — — — — — — — — — — —

fn parse_provider(slug: &str) -> Result<ProviderKind, AniError> {
    ProviderKind::from_slug(slug).ok_or(AniError::Metadata)
}

fn bearer_from_headers(headers: &HeaderMap) -> Result<String, AniError> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .ok_or(AniError::InvalidToken)?;
    let raw = auth.to_str().map_err(|_| AniError::InvalidToken)?;
    let token = raw
        .strip_prefix("Bearer ")
        .ok_or(AniError::InvalidToken)?
        .trim();
    if token.is_empty() {
        return Err(AniError::InvalidToken);
    }
    Ok(token.to_owned())
}

async fn post_auth_url(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Json(req): Json<AuthUrlRequest>,
) -> Result<Json<AuthUrlResponse>, AniError> {
    let kind = parse_provider(&provider)?;
    let pkce = req.pkce.into_pkce().ok_or(AniError::Metadata)?;
    let url = account::auth_url(&state, kind, &req.state, &pkce)?;
    Ok(Json(AuthUrlResponse { url }))
}

/// Push watch progress/status to a connected tracker. Returns the
/// upserted entry, or `null` when the show couldn't be mapped to the
/// provider (the renderer treats `null` as a no-op, not an error).
async fn post_update(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Json(req): Json<UpdateProgressRequest>,
) -> Result<Json<Option<ListEntry>>, AniError> {
    let kind = parse_provider(&provider)?;
    let bearer = bearer_from_headers(&headers)?;
    let tokens = account::tokens_from_bearer(&bearer);
    // Reject empty / typo'd payloads before they reach the upsert
    // (Codex P2 #3381617932).
    let update = account::build_entry_update(req.status.as_deref(), req.progress, req.score)?;
    let entry = account::push_progress(&state, kind, &tokens, &req.kitsu_id, update).await?;
    // Reflect the just-synced entry in the local cache (best-effort) so
    // the Watch Later rail drops a started title without a full resync
    // (Codex P2 #3412673593). Logic lives in commands to keep this
    // handler — and the file's CRAP — slim.
    account::write_through_after_update(&state, kind, &tokens, entry.as_ref()).await;
    Ok(Json(entry))
}

/// Explicit detail-page list edit: write the user's deliberate status /
/// progress to the tracker verbatim (no monotonic guard — they can
/// correct an over-count downward). Returns the upserted entry, or
/// `null` when the show couldn't be mapped to the provider.
async fn post_set(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Json(req): Json<SetEntryRequest>,
) -> Result<Json<Option<ListEntry>>, AniError> {
    let kind = parse_provider(&provider)?;
    let bearer = bearer_from_headers(&headers)?;
    let tokens = account::tokens_from_bearer(&bearer);
    let update = account::build_entry_update(req.status.as_deref(), req.progress, None)?;
    let entry = account::set_entry(&state, kind, &tokens, &req.kitsu_id, update).await?;
    // Force the explicit value into the cache (overwrites a higher
    // progress, unlike the monotonic mark-watched write-through) so the
    // rail/editor reflect a downward correction immediately.
    account::write_through_after_set(&state, kind, &tokens, entry.as_ref()).await;
    Ok(Json(entry))
}

/// Read the user's live current entry for a show so the detail-page
/// editor opens on the real tracker state. `null` when the show isn't on
/// the list or isn't mapped to the provider.
async fn get_entry(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Query(q): Query<EntryQuery>,
) -> Result<Json<Option<EntryView>>, AniError> {
    let kind = parse_provider(&provider)?;
    let bearer = bearer_from_headers(&headers)?;
    let tokens = account::tokens_from_bearer(&bearer);
    let current = account::get_entry(&state, kind, &tokens, &q.kitsu_id).await?;
    let view = current.map(|c| EntryView {
        status: account::status_to_snake(c.status).to_owned(),
        progress: c.progress_episodes,
    });
    Ok(Json(view))
}

/// Remove a show from the user's tracker list (editor "Remove"). 204
/// whether or not a row existed — idempotent.
async fn delete_entry(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Query(q): Query<EntryQuery>,
) -> Result<StatusCode, AniError> {
    let kind = parse_provider(&provider)?;
    let bearer = bearer_from_headers(&headers)?;
    let tokens = account::tokens_from_bearer(&bearer);
    account::remove_entry(&state, kind, &tokens, &q.kitsu_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn post_exchange_code(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Json(req): Json<ExchangeCodeRequest>,
) -> Result<Json<TokensResponse>, AniError> {
    let kind = parse_provider(&provider)?;
    let pkce = req.pkce.into_pkce().ok_or(AniError::Metadata)?;
    let tokens = account::exchange_code(&state, kind, &req.code, &pkce).await?;
    Ok(Json(tokens.into()))
}

/// Exchange a refresh token for a fresh access token (and rotated
/// refresh token). The renderer calls this when a persisted token has
/// expired but carries a refresh token — chiefly MAL's ~1h access
/// token — and re-persists the response via safeStorage, so the
/// provider stays connected instead of being forced back through
/// OAuth. Stateless: the renderer supplies the refresh token.
async fn post_refresh(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<TokensResponse>, AniError> {
    let kind = parse_provider(&provider)?;
    let tokens = account::refresh_tokens(&state, kind, &req.refresh_token).await?;
    Ok(Json(tokens.into()))
}

async fn post_me(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
) -> Result<Json<UserProfile>, AniError> {
    let kind = parse_provider(&provider)?;
    let bearer = bearer_from_headers(&headers)?;
    let tokens = account::tokens_from_bearer(&bearer);
    let profile = account::me(&state, kind, &tokens).await?;
    Ok(Json(profile))
}

async fn post_list(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<ListEntry>>, AniError> {
    // Codex P2 #3369972493: previously this accepted `user_id` from
    // the request body and used it as the cache-write owner. A
    // cross-origin page with its own valid bearer could choose any
    // other user_id and poison that target's local cache. Like the
    // cached read/delete paths, derive the owner from the bearer by
    // calling `me()` upstream — the bearer is the only identity
    // input the backend trusts.
    let kind = parse_provider(&provider)?;
    let bearer = bearer_from_headers(&headers)?;
    let tokens = account::tokens_from_bearer(&bearer);
    let profile = account::me(&state, kind, &tokens).await?;
    let entries = account::list_all_and_cache(&state, kind, &tokens, &profile.user_id).await?;
    Ok(Json(entries))
}

async fn get_cached_list(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Query(q): Query<DisconnectFallbackQuery>,
) -> Result<Json<Vec<ListEntry>>, AniError> {
    // Codex P2 #3372942241: previously this required a live `me()` to
    // succeed, which defeated the local cache the moment the user went
    // offline or AniList threw 5xx — exactly when the cached read is
    // most useful. Resolve through the shared helper so the bearer-
    // validated identity wins when reachable, with the renderer-only
    // internal-secret-gated fallback covering outage paths. Codex P1
    // #3369956138 (no caller-supplied user_id in the trusted path) is
    // still honoured: the fallback id is only consulted after me()
    // fails for offline/401/5xx AND the renderer secret authenticates
    // the request as coming from the Electron preload.
    let bearer = bearer_from_headers(&headers)?;
    let kind = parse_provider(&provider)?;
    let tokens = account::tokens_from_bearer(&bearer);
    let user_id =
        resolve_owner_user_id(&state, kind, &tokens, &headers, q.fallback_user_id).await?;
    let entries = account::cached_list(&state, kind, &user_id)?;
    Ok(Json(entries))
}

async fn delete_list_cache(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Query(q): Query<DisconnectFallbackQuery>,
) -> Result<StatusCode, AniError> {
    // Codex P2 #3370011855 + #3370096596 + #3372942241: shared resolver
    // so the same security gate covers the disconnect-delete and the
    // cached-read paths. Bearer-validated me() wins when reachable;
    // offline / 401 / 5xx falls through to the renderer-supplied id
    // gated by the internal secret.
    let bearer = bearer_from_headers(&headers)?;
    let kind = parse_provider(&provider)?;
    let tokens = account::tokens_from_bearer(&bearer);
    let user_id =
        resolve_owner_user_id(&state, kind, &tokens, &headers, q.fallback_user_id).await?;
    account::clear_cache(&state, kind, &user_id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Provider-wide cache wipe — no bearer, no user_id, just the
/// renderer-only internal secret. Codex P2 #3371658227: the
/// orphan-token disconnect path (hydrate found the token file
/// unreadable, so the store has no account) has no `user_id` to pass
/// to the per-user delete and no live bearer to authenticate one.
/// Gating on the internal secret keeps cross-origin tabs out — only
/// the Electron renderer learned the 32-byte secret at startup.
async fn delete_list_cache_all(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, AniError> {
    state.internal_secret.validate_header(&headers)?;
    let kind = parse_provider(&provider)?;
    account::clear_provider_cache(&state, kind)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[path = "account_test.rs"]
mod tests;
