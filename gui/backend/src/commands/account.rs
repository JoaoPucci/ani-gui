//! Account integration handlers — the per-route bodies the
//! [`crate::api::account`] submodule mounts.
//!
//! Statelessness contract: the backend holds NO session state. Every
//! handler that talks to a provider receives the bearer in the request
//! header (extracted before reaching these functions). Tokens live in
//! Electron's `safeStorage` on disk + renderer memory; the Rust process
//! is a pure proxy + cache.
//!
//! Why: the Rust binary is also the test target (`cargo test`) and
//! can't depend on an Electron runtime to read the keychain. Pushing
//! token persistence to Electron keeps the backend deterministic and
//! testable.
//!
//! Token lifecycle (see `.planning/account-integration.md` §3.4):
//!
//! 1. Renderer → backend `auth_url` → returns URL + state + pkce
//! 2. Renderer → Electron main: open browser, listen on :53682
//! 3. Browser redirects to localhost callback with `?code=&state=`
//! 4. Renderer → backend `exchange_code` → returns Tokens
//! 5. Renderer → Electron main: safeStorage-encrypt + persist
//! 6. Renderer → backend `me` / `list_all` with `Authorization: Bearer`

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::account::pkce::Pkce;
use crate::account::provider::{
    CurrentEntry, EntryUpdate, ListEntry, ProviderKind, ProviderMediaId, Tokens, UserListProvider,
    UserProfile,
};
use crate::account::status::ListStatus;
use crate::app::AppState;
use crate::cache::SqlitePool;
use crate::error::{AniError, Result};
use crate::meta::anilist_user::AniListProvider;
use crate::meta::mal_user::MalProvider;

/// The per-show async mutex callers hold across the read + upsert.
type ShowLock = Arc<tokio::sync::Mutex<()>>;
/// Process-wide map of `(provider, Kitsu id)` → its [`ShowLock`]. Keyed on
/// the Kitsu id (not the native media id) so the lock can be taken before
/// `resolve_native_media_id`, serializing the whole resolve→read/write.
type ShowLockMap = Mutex<HashMap<(ProviderKind, String), ShowLock>>;

/// Per-(provider, native media id) serialization for write-back.
///
/// `push_progress` reads `current_progress` before the upsert, and the
/// route callers fire `syncWatchedToTrackers` without awaiting — so two
/// fan-out calls for the same show can both read the same count and the
/// later-landing lower write would regress it (Codex P2 #3387237642).
/// Providers only upsert (no compare-and-set), so the fix is to hold a
/// per-show lock across the read + reconcile + write, making the
/// monotonic guard atomic. The map indirection keeps distinct shows
/// concurrent. Process-wide on [`AppState`] so it spans the separate
/// HTTP requests each un-awaited write becomes; cloned `Arc` is cheap.
///
/// The map grows one entry per show ever written this run — negligible
/// for a desktop session (a binge tops out in the low hundreds), so it
/// isn't pruned.
#[derive(Clone, Default)]
pub struct AccountWriteLocks {
    inner: Arc<ShowLockMap>,
}

impl AccountWriteLocks {
    /// An empty lock map. Production builds one at boot and clones the
    /// cheap `Arc` into every `AppState`; tests `new()` per fixture.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The lock guarding writes to one `(provider, Kitsu id)`. Taken before
    /// native-id resolution so a seed read and a mark-watched write for the
    /// same show serialize across the whole resolve→read/write. The brief
    /// std-mutex section only swaps the `Arc`; the returned async mutex is
    /// what the caller holds across the network round-trips.
    pub(crate) fn for_show(&self, kind: ProviderKind, kitsu_id: &str) -> ShowLock {
        let mut map = self.inner.lock().expect("account write-lock map poisoned");
        Arc::clone(map.entry((kind, kitsu_id.to_owned())).or_default())
    }
}

/// Build a [`UserListProvider`] for the requested provider kind. Used
/// by every route handler that calls into a concrete provider. Takes
/// the whole [`AppState`] so MAL can pull the process-wide refresh
/// coalesce cache off it — without that shared cache, two concurrent
/// handlers each build their own `MalProvider` with its own mutex and
/// both POST the same stale refresh token (Codex P2 #3379969316).
///
/// PR #4+ (in-house) eventually adds the third branch. Today InHouse
/// returns `None` — the routes that dispatch by slug guard against
/// unsupported kinds at the slug-parse step, but defending here too
/// keeps the dispatcher honest.
#[must_use]
pub fn provider_for_kind(
    state: &Arc<AppState>,
    kind: ProviderKind,
) -> Option<Box<dyn UserListProvider>> {
    let client = state.proxy_http.clone();
    match kind {
        ProviderKind::AniList => Some(Box::new(AniListProvider::new(client))),
        ProviderKind::MyAnimeList => Some(Box::new(MalProvider::new(
            client,
            state.mal_refresh.clone(),
        ))),
        ProviderKind::InHouse => None,
    }
}

/// Build the authorize-URL the renderer hands to Electron's
/// `shell.openExternal`. The renderer must persist `state` + the PKCE
/// pair so the exchange-code handler can verify them later (CSRF).
///
/// `state` is generated by the renderer (CSRF token). `pkce` is
/// generated by the renderer per the provider's method requirement
/// (plain for MAL, S256 for the future in-house, ignored by AniList
/// but generated for trait symmetry).
pub fn auth_url(
    state: &Arc<AppState>,
    kind: ProviderKind,
    csrf: &str,
    pkce: &Pkce,
) -> Result<String> {
    let Some(provider) = provider_for_kind(state, kind) else {
        return Err(AniError::Metadata);
    };
    provider.auth_url(pkce, csrf)
}

/// Exchange an OAuth authorization code for tokens. Renderer then
/// passes the returned [`Tokens`] to Electron main for `safeStorage`
/// encryption.
pub async fn exchange_code(
    state: &Arc<AppState>,
    kind: ProviderKind,
    code: &str,
    pkce: &Pkce,
) -> Result<Tokens> {
    let Some(provider) = provider_for_kind(state, kind) else {
        return Err(AniError::Metadata);
    };
    provider.exchange_code(code, pkce).await
}

/// Exchange a refresh token for a fresh token set. The renderer calls
/// this when a persisted token has expired but is still refreshable
/// (MAL's ~1h access token against its long-lived refresh token), then
/// re-persists the result. AniList's provider has no real refresh and
/// returns an error, which the renderer treats as "fall back to
/// reauth".
pub async fn refresh_tokens(
    state: &Arc<AppState>,
    kind: ProviderKind,
    refresh_token: &str,
) -> Result<Tokens> {
    let Some(provider) = provider_for_kind(state, kind) else {
        return Err(AniError::Metadata);
    };
    provider.refresh(refresh_token).await
}

/// Fetch the authenticated user's profile. Routes use this to populate
/// the AccountChip avatar + `/account` page stats.
pub async fn me(state: &Arc<AppState>, kind: ProviderKind, tokens: &Tokens) -> Result<UserProfile> {
    let Some(provider) = provider_for_kind(state, kind) else {
        return Err(AniError::Metadata);
    };
    provider.me(tokens).await
}

/// Fetch the full user list (all statuses) and write it through the
/// cache. Returns the freshly-fetched entries (not the cache contents
/// — caller can fan them out to surfaces without an extra read).
pub async fn list_all_and_cache(
    state: &Arc<AppState>,
    kind: ProviderKind,
    tokens: &Tokens,
    user_id: &str,
) -> Result<Vec<ListEntry>> {
    let Some(provider) = provider_for_kind(state, kind) else {
        return Err(AniError::Metadata);
    };
    let entries = provider.list_all(tokens).await?;
    write_through_cache(&state.cache_pool, kind, user_id, &entries)?;
    Ok(entries)
}

/// Read the cached list. Used by PR #2's home Watch Later rail to
/// avoid an upstream round-trip on every paint.
pub fn cached_list(
    state: &Arc<AppState>,
    kind: ProviderKind,
    user_id: &str,
) -> Result<Vec<ListEntry>> {
    crate::account::cache::list_entries(&state.cache_pool, kind, user_id)
}

/// Drop every cached row for `(kind, user_id)` — called on disconnect.
pub fn clear_cache(state: &Arc<AppState>, kind: ProviderKind, user_id: &str) -> Result<()> {
    crate::account::cache::clear_user(&state.cache_pool, kind, user_id)
}

/// Drop every cached row for `kind` regardless of `user_id`. Codex P2
/// #3371658227: the renderer's safeStorage-orphan-disconnect path has
/// no `user_id` to pass — the token file existed but the keychain
/// couldn't decrypt it on hydrate — so we can't run the per-user
/// clear. The API boundary still gates this on the renderer-only
/// internal secret so a cross-origin tab can't wipe a stranger's
/// cache by guessing nothing.
pub fn clear_provider_cache(state: &Arc<AppState>, kind: ProviderKind) -> Result<()> {
    crate::account::cache::clear_provider(&state.cache_pool, kind)
}

fn write_through_cache(
    pool: &SqlitePool,
    kind: ProviderKind,
    user_id: &str,
    entries: &[ListEntry],
) -> Result<()> {
    crate::account::cache::write_entries(pool, kind, user_id, entries)
}

/// Write a single just-synced entry back into the cache so the local
/// snapshot reflects the change without a full resync (Codex P2
/// #3412673593). Leaves the rest of the user's cached list intact.
pub fn upsert_cached_entry(
    state: &Arc<AppState>,
    kind: ProviderKind,
    user_id: &str,
    entry: &ListEntry,
) -> Result<()> {
    crate::account::cache_upsert::upsert_entry(&state.cache_pool, kind, user_id, entry)
}

/// Decode the `Authorization: Bearer <token>` header value into a
/// [`Tokens`] envelope. `expires_at_epoch_s` is zero because the
/// header doesn't carry expiry — that's tracked in the renderer's
/// token store and acted on (refresh / reauth) before the handler is
/// called. PR #4 adds a richer header that round-trips the expiry.
#[must_use]
pub fn tokens_from_bearer(bearer: &str) -> Tokens {
    Tokens {
        access_token: bearer.to_owned(),
        refresh_token: None,
        expires_at_epoch_s: 0,
    }
}

/// Build a fresh PKCE pair appropriate for the provider's method
/// requirement. AniList ignores PKCE but we still pick S256 for trait
/// symmetry; MAL requires `plain`.
#[must_use]
pub fn pkce_for_kind(kind: ProviderKind) -> Pkce {
    match kind {
        ProviderKind::MyAnimeList => Pkce::new_plain(),
        ProviderKind::AniList | ProviderKind::InHouse => Pkce::new_s256(),
    }
}

/// Translate the unified [`ListStatus`] into the snake_case string the
/// SQL cache stores. Mirrors `#[serde(rename_all = "snake_case")]`
/// without round-tripping through JSON.
#[must_use]
pub fn status_to_snake(s: ListStatus) -> &'static str {
    match s {
        ListStatus::Planning => "planning",
        ListStatus::Watching => "watching",
        ListStatus::Completed => "completed",
        ListStatus::Paused => "paused",
        ListStatus::Dropped => "dropped",
        ListStatus::Rewatching => "rewatching",
    }
}

/// Inverse of [`status_to_snake`]. Used by [`crate::account::cache`]
/// when reading rows back into typed [`ListEntry`].
#[must_use]
pub fn status_from_snake(s: &str) -> Option<ListStatus> {
    match s {
        "planning" => Some(ListStatus::Planning),
        "watching" => Some(ListStatus::Watching),
        "completed" => Some(ListStatus::Completed),
        "paused" => Some(ListStatus::Paused),
        "dropped" => Some(ListStatus::Dropped),
        "rewatching" => Some(ListStatus::Rewatching),
        _ => None,
    }
}

/// Cap on the renderer-supplied Watch-Later bridge batch. The home
/// rail's largest plausible Plan-to-Watch is a few hundred titles;
/// 500 leaves comfortable headroom while bounding the fan-out cost
/// per request to a fixed worst case. Codex P1 #3373789621: under
/// the permissive CORS layer a cross-origin page could otherwise
/// POST an unbounded `mal_ids` array and burn N concurrent Kitsu
/// requests per call.
pub const WATCH_LATER_BRIDGE_MAX_IDS: usize = 500;

/// Concurrent in-flight Kitsu `/mappings` lookups during the
/// Watch-Later bridge. Codex P2 #3373969321: `lookup_by_mal_id` is
/// uncached so a single rail load could fire up to
/// [`WATCH_LATER_BRIDGE_MAX_IDS`] requests in parallel and trip
/// Kitsu's upstream rate limit (the AniList trending bridge tops
/// out at 25 — well below the throttle threshold — and never hit
/// this; the rail's larger batch exposes it). 8 is well under the
/// observed safe parallelism for `/mappings` and keeps the
/// worst-case rail wall-clock under ~5s while still much faster
/// than serial.
const WATCH_LATER_BRIDGE_CONCURRENCY: usize = 8;

/// Bridge a list of MAL ids to Kitsu refs, preserving input order
/// and dropping ids Kitsu can't map. Used by the home page's Watch
/// Later rail (plan §6.6) so cached `user_list_cache` rows render
/// with the same Kitsu metadata + availability filter as the rest
/// of the home.
///
/// Concurrency is capped at [`WATCH_LATER_BRIDGE_CONCURRENCY`] via
/// `buffered(N)` so we don't queue up to 500 outbound Kitsu
/// requests at once (Codex P2 #3373969321).
///
/// Truncates `mal_ids` at [`WATCH_LATER_BRIDGE_MAX_IDS`] before
/// fan-out — the upstream gate (route handler) rejects oversize
/// batches outright, this cap is belt-and-suspenders so a missing
/// route-level check can't be exploited.
///
/// # Errors
/// Never fails the whole batch — individual lookup failures drop the
/// entry from the output. Empty input → empty output.
pub async fn kitsu_for_mal_ids(
    state: &Arc<AppState>,
    mal_ids: Vec<u32>,
) -> Vec<crate::meta::kitsu::KitsuAnimeRef> {
    kitsu_for_mal_ids_with_anilist_base(state, mal_ids, None).await
}

/// [`kitsu_for_mal_ids`] with the AniList endpoint override exposed
/// for tests (the fallback hop below hits AniList). Production passes
/// `None` via the public wrapper.
pub(crate) async fn kitsu_for_mal_ids_with_anilist_base(
    state: &Arc<AppState>,
    mal_ids: Vec<u32>,
    anilist_base: Option<&str>,
) -> Vec<crate::meta::kitsu::KitsuAnimeRef> {
    use futures_util::stream::{self, StreamExt};

    let bounded = mal_ids
        .into_iter()
        .take(WATCH_LATER_BRIDGE_MAX_IDS)
        .collect::<Vec<_>>();
    // Phase 1: the direct Kitsu lookup, one slot per input id so the
    // fallback fill below preserves input order.
    let mut slots: Vec<Option<crate::meta::kitsu::KitsuAnimeRef>> =
        stream::iter(bounded.iter().copied())
            .map(|mal_id| {
                let kitsu = state.kitsu.clone();
                async move { kitsu.lookup_by_mal_id(mal_id).await.ok().flatten() }
            })
            .buffered(WATCH_LATER_BRIDGE_CONCURRENCY)
            .collect()
            .await;
    // Phase 2: Kitsu hasn't MAL-mapped the misses (fresh seasonal
    // titles) — resolve ALL of them to AniList ids in one batched
    // Page(idMal_in:) query per 50-chunk (Codex P2 #3565216298: the
    // per-miss Media(idMal:) call could burn AniList's 30 req/min
    // budget on a single big rail load), then look each up under
    // Kitsu's anilist/anime mapping. Same gap resolve_native_media_id
    // works around in the write direction; without this the
    // just-written entry never renders in the rail. A failed batch
    // degrades to dropping those cards for this load, as before.
    let misses: Vec<(usize, u32)> = slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.is_none())
        .map(|(i, _)| (i, bounded[i]))
        .collect();
    if !misses.is_empty() {
        let miss_ids: Vec<u32> = misses.iter().map(|&(_, mal_id)| mal_id).collect();
        let mal_to_anilist =
            crate::meta::anilist::media_ids_for_mals(&state.proxy_http, &miss_ids, anilist_base)
                .await
                .unwrap_or_default();
        let filled: Vec<(usize, Option<crate::meta::kitsu::KitsuAnimeRef>)> = stream::iter(misses)
            .map(|(i, mal_id)| {
                let kitsu = state.kitsu.clone();
                let anilist_id = mal_to_anilist.get(&mal_id).copied();
                async move {
                    let Some(anilist_id) = anilist_id else {
                        return (i, None);
                    };
                    (
                        i,
                        kitsu.lookup_by_anilist_id(anilist_id).await.ok().flatten(),
                    )
                }
            })
            .buffered(WATCH_LATER_BRIDGE_CONCURRENCY)
            .collect()
            .await;
        for (i, r) in filled {
            slots[i] = r;
        }
    }
    slots.into_iter().flatten().collect()
}

/// Resolve a show's Kitsu id into the provider-native media id needed
/// by `update_entry`: MAL's anime id for MyAnimeList, AniList's
/// numeric `mediaId` for AniList. `Ok(None)` when no mapping route
/// reaches the provider — a non-error "nothing to push" so the
/// mark-watched fan-out skips it rather than reporting a failure.
/// `anilist_base` overrides the AniList endpoint in tests; production
/// passes `None`.
///
/// The primary route pivots on the show's MAL id from Kitsu's
/// mappings (MAL directly; AniList via `Media(idMal:)`). But fresh
/// seasonal shows often carry only the `anilist/anime` mapping —
/// Yani Neko (Kitsu 50551) stayed unwritable on every tracker for
/// months that way — so each provider falls back to that direct
/// mapping when the MAL pivot yields nothing: AniList uses it as-is,
/// MAL bridges it through AniList's `Media(id:){idMal}`. The
/// fallbacks only fire where the primary route returns `None`, so
/// shows that resolved before resolve to the same ids.
///
/// This is the live id-lookup foundation for write-back: both
/// providers' `update_entry` upsert, so resolving the native id is
/// all that's needed to push progress to a show — including one not
/// yet on the user's list (the basis for currently-watching tracking).
pub(crate) async fn resolve_native_media_id(
    state: &Arc<AppState>,
    kind: ProviderKind,
    kitsu_id: &str,
    anilist_base: Option<&str>,
) -> Result<Option<ProviderMediaId>> {
    let ids = state.kitsu.external_ids_for_kitsu_id(kitsu_id).await?;
    match kind {
        // MAL's anime id IS the mapped MAL id; else anilist mapping →
        // AniList's recorded idMal.
        ProviderKind::MyAnimeList => {
            if let Some(mal_id) = ids.mal {
                return Ok(Some(ProviderMediaId(mal_id)));
            }
            let Some(anilist_id) = ids.anilist else {
                return Ok(None);
            };
            let mal_id = crate::meta::anilist::mal_id_for_media_id(
                &state.proxy_http,
                anilist_id,
                anilist_base,
            )
            .await?;
            Ok(mal_id.map(ProviderMediaId))
        }
        // AniList keys on its own numeric mediaId: MAL bridge first
        // (unchanged primary), direct anilist/anime mapping second.
        ProviderKind::AniList => {
            if let Some(mal_id) = ids.mal {
                let media_id =
                    crate::meta::anilist::media_id_for_mal(&state.proxy_http, mal_id, anilist_base)
                        .await?;
                if media_id.is_some() {
                    return Ok(media_id.map(ProviderMediaId));
                }
            }
            Ok(ids.anilist.map(ProviderMediaId))
        }
        // In-house provider has no external id space yet.
        ProviderKind::InHouse => Ok(None),
    }
}

/// Build a validated [`EntryUpdate`] from the renderer's wire fields.
/// Rejects two malformed shapes (Codex P2 #3381617932):
///
/// - an all-absent update (no status, progress, or score) — since
///   both providers' `update_entry` upsert, an empty update would
///   create a list row with upstream defaults instead of being a
///   no-op;
/// - a `status` string that isn't a recognized unified value — a typo
///   silently dropped to `None` would make a bad request look like a
///   successful (but empty) update.
///
/// Both surface as [`AniError::Metadata`], matching how the account
/// routes already treat malformed input (bad provider slug).
pub fn build_entry_update(
    status: Option<&str>,
    progress_episodes: Option<u32>,
    score_0_to_100: Option<u8>,
) -> Result<EntryUpdate> {
    let status = match status {
        None => None,
        Some(s) => Some(status_from_snake(s).ok_or(AniError::Metadata)?),
    };
    if status.is_none() && progress_episodes.is_none() && score_0_to_100.is_none() {
        return Err(AniError::Metadata);
    }
    Ok(EntryUpdate {
        status,
        progress_episodes,
        score_0_to_100,
        repeat_count: None,
    })
}

/// Reconcile a write against the tracker's `current` list entry so the
/// write-back stays monotonic and status-preserving, returning the
/// fields still worth sending (or `None` when nothing actionable
/// remains, so the caller skips the write entirely).
///
/// Progress (Codex P1 #3386909281): if the requested count wouldn't
/// advance `current` (a replay / the Previous button) the progress field
/// is dropped — never decrease the count.
///
/// Status: the fan-out only ever sends `Completed` (a finished finale),
/// otherwise it sends no status. This helper fills the rest in:
///   - a `Completed` write is always kept, even at unchanged progress —
///     finishing a series is a valid forward move (Codex P2 #3387051891);
///   - a status-less *advancing* progress write promotes a planning row
///     (or a not-yet-on-the-list show) to `Watching` so a Plan-to-Watch
///     title leaves Watch Later (Codex P2 #3387383171), but leaves any
///     other status untouched — preserving `Rewatching`/`Completed`/
///     `Paused` (Codex P2 #3387319861);
///   - a non-advancing `Watching` is dropped (it could only downgrade or
///     no-op).
pub fn reconcile_monotonic(
    mut update: EntryUpdate,
    current: Option<CurrentEntry>,
) -> Option<EntryUpdate> {
    let current_progress = current.map(|c| c.progress_episodes);
    // Whether this write carries a watch event at all — captured before
    // the non-advance branch may strip the progress field, so the
    // planning promotion below still fires on a non-advancing write.
    let is_watch_event = update.progress_episodes.is_some();
    let advances = match (current_progress, update.progress_episodes) {
        (Some(c), Some(p)) => p > c,
        _ => true,
    };
    if !advances {
        update.progress_episodes = None;
        if update.status == Some(ListStatus::Watching) {
            update.status = None;
        }
    }
    // Preserve a rewatching/repeating row at the finale (Codex P2
    // #3415780486): the fan-out sends Completed when a finished series'
    // last episode is watched, but completing a row that's mid-rewatch
    // would clear AniList REPEATING / MAL is_rewatching. Drop the
    // Completed status so the rewatch state survives; any advancing
    // progress field still flows below.
    if update.status == Some(ListStatus::Completed)
        && current.map(|c| c.status) == Some(ListStatus::Rewatching)
    {
        update.status = None;
    }
    // Promote a planning / not-yet-listed row to Watching on ANY watch
    // event, advancing or not (Codex P2 #3387568872: a planning row
    // already at the same/higher count must still leave Watch Later),
    // but leave every other status untouched — preserving
    // rewatching/paused/completed (Codex P2 #3387319861).
    if is_watch_event && update.status.is_none() {
        let promote = match current {
            None => true,
            Some(c) => c.status == ListStatus::Planning,
        };
        if promote {
            update.status = Some(ListStatus::Watching);
        }
    }
    let empty = update.status.is_none()
        && update.progress_episodes.is_none()
        && update.score_0_to_100.is_none()
        && update.repeat_count.is_none();
    if empty {
        None
    } else {
        Some(update)
    }
}

/// Push `update` (progress / status / score) to a connected tracker
/// for the show identified by its Kitsu id. Called once per connected
/// provider by the mark-watched fan-out, each with that provider's
/// bearer.
///
/// Returns `Ok(None)` when the show can't be mapped to the provider
/// (no MAL mapping, or AniList doesn't index it) — a non-error "nothing
/// to push" so the fan-out treats it as a skip, not a failure.
/// `Ok(Some(entry))` carries the upserted entry (both providers'
/// `update_entry` create the list row if absent — the basis for
/// currently-watching tracking).
pub async fn push_progress(
    state: &Arc<AppState>,
    kind: ProviderKind,
    tokens: &Tokens,
    kitsu_id: &str,
    update: EntryUpdate,
) -> Result<Option<ListEntry>> {
    push_progress_with_anilist_base(state, kind, tokens, kitsu_id, update, None).await
}

/// Inner `push_progress` with the AniList endpoint override threaded
/// through to `resolve_native_media_id`. Production calls
/// `push_progress` (base `None`); tests pass a wiremock URI.
async fn push_progress_with_anilist_base(
    state: &Arc<AppState>,
    kind: ProviderKind,
    tokens: &Tokens,
    kitsu_id: &str,
    update: EntryUpdate,
    anilist_base: Option<&str>,
) -> Result<Option<ListEntry>> {
    // Take the per-show lock BEFORE resolving the native id, keyed on the
    // Kitsu id, so this write serializes with the editor's seed read (which
    // takes the same lock) across the whole resolve→read/write — closing the
    // window where a read could resolve, win the lock, and read a value this
    // write is about to change (Codex P2 #3428252253). Held across the GET +
    // upsert since providers only upsert (no compare-and-set).
    let show_lock = state.account_write_locks.for_show(kind, kitsu_id);
    let _write_guard = show_lock.lock().await;
    let Some(native) = resolve_native_media_id(state, kind, kitsu_id, anilist_base).await? else {
        return Ok(None);
    };
    let Some(provider) = provider_for_kind(state, kind) else {
        return Err(AniError::Metadata);
    };
    push_progress_via(state, kind, provider.as_ref(), tokens, native, update).await
}

/// The monotonic write-back with the provider injected, so tests drive
/// it against a wiremock-backed provider (the public path builds the
/// real one). The per-show lock is held by the caller
/// ([`push_progress_with_anilist_base`]) across resolve → this read →
/// reconcile → write → cache write-through, so the read-then-write
/// monotonic guard is atomic (Codex P2 #3387237642): the route callers
/// fire syncWatchedToTrackers without awaiting, so two writes for the
/// same show would otherwise both read the same count and the
/// later-landing lower one would regress it.
async fn push_progress_via(
    state: &Arc<AppState>,
    kind: ProviderKind,
    provider: &dyn UserListProvider,
    tokens: &Tokens,
    native: ProviderMediaId,
    update: EntryUpdate,
) -> Result<Option<ListEntry>> {
    // Monotonic guard (Codex P1 #3386909281 / P2 #3387051891): reconcile
    // the write against the tracker's current count so a replay can't
    // regress progress or downgrade a finished show, while still letting
    // a completion through at unchanged progress. Reading current
    // progress costs one bearer-scoped GET per write — only when the
    // write carries progress, and the only authoritative source (the
    // local cache is empty until the user syncs their list).
    let update = if update.progress_episodes.is_some() {
        let current = provider.current_entry(tokens, native).await?;
        match reconcile_monotonic(update, current) {
            Some(reconciled) => reconciled,
            None => return Ok(None),
        }
    } else {
        update
    };
    let entry = provider.update_entry(tokens, native, update).await?;
    // Write the cache through HERE, under the per-show lock — not in the
    // route afterwards. Deferring it released the lock first, so a stale
    // mark-watched write-through could land after an explicit downward
    // correction and clobber it (Codex P2 #3423108941 / #3423044438).
    // Inside the lock it's serialized with the explicit editor's
    // force-upsert, so no stale write lingers; the monotonic upsert still
    // guards two racing mark-watched writes. Best-effort.
    if let Ok(profile) = provider.me(tokens).await {
        let _ = upsert_cached_entry(state, kind, &profile.user_id, &entry);
    }
    Ok(Some(entry))
}

#[cfg(test)]
#[path = "account_test.rs"]
mod tests;
