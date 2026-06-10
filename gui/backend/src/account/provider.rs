//! `UserListProvider` trait — every concrete provider (AniList today,
//! MAL in PR #3, future in-house) implements this.
//!
//! Read methods (`me`, `list_all`) implement in PR #1.
//! Write methods (`update_entry`, `delete_entry`) stub-return
//! `AniError::Metadata` in PR #1; concrete impls land in PR #4
//! (write-back).
//!
//! Timestamps are i64 epoch seconds to avoid pulling `chrono` into the
//! backend. AniList returns `Int` timestamps natively; MAL returns
//! ISO-8601 strings parsed at the wire boundary into epoch seconds.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::account::pkce::Pkce;
use crate::account::status::ListStatus;
use crate::error::Result;

/// Which provider an instance belongs to.
///
/// Codex P2 #3369980190: serde rename targets match `slug()` exactly,
/// so a `ProviderKind::AniList` field on a serialized `UserProfile` /
/// `ListEntry` round-trips through the renderer's `Provider` type
/// (`'anilist' | 'mal' | 'inhouse'`). The default snake_case derive
/// would have emitted `"ani_list"` and silently broken
/// `accountStore.byProvider` lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderKind {
    /// `anilist.co`. GraphQL. 1-year JWT. No refresh tokens.
    #[serde(rename = "anilist")]
    AniList,
    /// `myanimelist.net`. REST. 1-hour access tokens, 1-month refresh.
    #[serde(rename = "mal")]
    MyAnimeList,
    /// Reserved for a future ani-gui-as-its-own-provider implementation.
    #[serde(rename = "inhouse")]
    InHouse,
}

impl ProviderKind {
    /// URL slug used in route paths (`/api/account/connect/anilist`).
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::AniList => "anilist",
            Self::MyAnimeList => "mal",
            Self::InHouse => "inhouse",
        }
    }

    /// Parse a slug back into a variant. `None` for unknown values.
    #[must_use]
    pub fn from_slug(s: &str) -> Option<Self> {
        match s {
            "anilist" => Some(Self::AniList),
            "mal" => Some(Self::MyAnimeList),
            "inhouse" => Some(Self::InHouse),
            _ => None,
        }
    }
}

/// Provider-scoped media id. Prevents accidentally treating an AniList
/// id (Media.id) as a MAL id (anime_id) elsewhere in the codebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderMediaId(pub u32);

/// OAuth tokens for one provider session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    /// Bearer used in `Authorization: Bearer …` upstream calls.
    pub access_token: String,
    /// Long-lived refresh token. `None` for AniList (no refresh flow).
    pub refresh_token: Option<String>,
    /// Unix epoch seconds when the access token stops being valid.
    /// AniList: ~1 year from issuance. MAL: 1 hour.
    pub expires_at_epoch_s: i64,
}

impl Tokens {
    /// True when `now_epoch_s` is within `lead_s` seconds of expiry.
    /// MAL uses this with a 5-minute lead to pre-emptively refresh
    /// before an access token expires mid-request.
    #[must_use]
    pub fn expires_within(&self, now_epoch_s: i64, lead_s: i64) -> bool {
        self.expires_at_epoch_s <= now_epoch_s + lead_s
    }
}

/// Aggregate stats shown in the account popover / `/account` page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    /// Total entries on the user's anime list.
    pub anime_count: u32,
    /// Mean score on the 0..=10 scale (display only — converted from
    /// each provider's native scale at the wire boundary).
    pub mean_score_0_to_10: Option<f32>,
}

/// User profile populated from the provider's `me`-style endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    /// Provider this profile belongs to.
    pub provider: ProviderKind,
    /// Stable identifier on the provider (used as a cache key).
    pub user_id: String,
    /// Display name.
    pub username: String,
    /// Avatar URL. `None` when the user hasn't uploaded one.
    pub avatar_url: Option<String>,
    /// Aggregate stats. `None` when the provider doesn't expose them.
    pub stats: Option<UserStats>,
}

/// One row in the user's anime list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListEntry {
    /// Source provider for this row.
    pub provider: ProviderKind,
    /// Provider-native media id (AniList.id or MAL anime_id).
    pub media_id: ProviderMediaId,
    /// Cross-provider bridge — AniList exposes this as `idMal`; MAL
    /// returns the same value as `media_id`. The Watch Later rail
    /// dedupes across providers on this id and the home bridge to
    /// Kitsu also goes through it (Kitsu's `mappings` endpoint takes
    /// `filter[externalSite]=myanimelist/anime`).
    pub mal_id: Option<u32>,
    /// Unified status.
    pub status: ListStatus,
    /// Episodes watched.
    pub progress_episodes: u32,
    /// Score on the 0..=100 unified scale. `None` when unrated.
    pub score_0_to_100: Option<u8>,
    /// Last update timestamp from the provider (unix epoch seconds).
    pub updated_at_epoch_s: i64,
    /// Display title — fallback only; cards render with Kitsu metadata.
    pub title: String,
}

/// Update payload for write-back (PR #4).
#[derive(Debug, Clone, Default)]
pub struct EntryUpdate {
    /// New status — leave `None` to keep current.
    pub status: Option<ListStatus>,
    /// New episodes-watched count — leave `None` to keep current.
    pub progress_episodes: Option<u32>,
    /// New score (0..=100 unified scale) — leave `None` to keep current.
    pub score_0_to_100: Option<u8>,
    /// New repeat count for rewatches — leave `None` to keep current.
    pub repeat_count: Option<u32>,
}

/// Trait every concrete user-list provider implements.
#[async_trait]
pub trait UserListProvider: Send + Sync {
    /// Which provider this instance represents.
    fn kind(&self) -> ProviderKind;

    /// Build the URL the user's OS browser opens for the consent flow.
    /// `state` is the CSRF token the loopback callback server verifies
    /// on redirect. Returns `Err(AniError::Metadata)` when the provided
    /// PKCE configuration is unsupported by the upstream (MAL rejects
    /// `S256` — surfacing the misuse here lets the route handler return
    /// a clean 4xx instead of panicking or silently emitting an empty
    /// URL that fails later in the connect flow).
    fn auth_url(&self, pkce: &Pkce, state: &str) -> Result<String>;

    /// Trade an authorization code for tokens after the user approves.
    async fn exchange_code(&self, code: &str, pkce: &Pkce) -> Result<Tokens>;

    /// Trade a refresh token for fresh tokens. AniList returns
    /// `AniError::Metadata` — it has no refresh flow.
    async fn refresh(&self, refresh_token: &str) -> Result<Tokens>;

    /// Fetch the authenticated user's profile.
    async fn me(&self, tokens: &Tokens) -> Result<UserProfile>;

    /// Fetch the full user list (all statuses) in one logical call.
    /// Providers paginate internally.
    async fn list_all(&self, tokens: &Tokens) -> Result<Vec<ListEntry>>;

    /// Write `update` to one media id. PR #1 stubs to
    /// `Err(AniError::Metadata)`; PR #4 implements.
    async fn update_entry(
        &self,
        tokens: &Tokens,
        id: ProviderMediaId,
        update: EntryUpdate,
    ) -> Result<ListEntry>;

    /// Remove an entry. PR #1 stubs to `Err(AniError::Metadata)`;
    /// PR #4 implements.
    async fn delete_entry(&self, tokens: &Tokens, id: ProviderMediaId) -> Result<()>;

    /// Current watched-episode count for `id` on the authenticated
    /// user's list, or `None` when the show isn't on their list yet.
    /// Bearer-scoped single read — used to keep write-back monotonic
    /// so replaying an earlier episode never regresses the tracker's
    /// cumulative progress (Codex P1 #3386909281).
    async fn current_progress(&self, tokens: &Tokens, id: ProviderMediaId) -> Result<Option<u32>>;
}
