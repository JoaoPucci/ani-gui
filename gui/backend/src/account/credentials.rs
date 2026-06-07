//! OAuth client credentials.
//!
//! Public by design — these ship in the distributed binary and can be
//! extracted via `strings`. The "secret" label is a misnomer for native
//! OAuth clients: AniList accepts the same tradeoff that Aniyomi,
//! Hayase, and MAL-Sync make publicly, and MAL sidesteps it entirely
//! by issuing client_id only for App Type "Other" (PKCE handles
//! authentication, no shared secret exists).
//!
//! Do NOT treat these as secrets in code review, git history, or
//! security audits. They are public credentials by construction.
//!
//! Rotation: regenerate the AniList API client at
//! <https://anilist.co/settings/developer> and the MAL app at
//! <https://myanimelist.net/apiconfig>, then update the constants
//! below. No build-time injection needed — fork users can replace
//! these in their own builds.

/// AniList API client id (numeric, public).
pub const ANILIST_CLIENT_ID: &str = "43143";

/// AniList API client secret. Public by construction (binary-extractable);
/// see module doc-comment.
pub const ANILIST_CLIENT_SECRET: &str = "B5ay8XNwslA819aukVNC8ejQBvUXtpuLiPf1yvL7";

/// AniList registered redirect URI. Single value — AniList allows
/// only one per API client, so loopback is the canonical path.
pub const ANILIST_REDIRECT_URI: &str = "http://localhost:53682/callback";

/// AniList GraphQL endpoint (data API).
pub const ANILIST_API: &str = "https://graphql.anilist.co";

/// AniList OAuth authorize URL (browser-side).
pub const ANILIST_AUTH_URL: &str = "https://anilist.co/api/v2/oauth/authorize";

/// AniList OAuth token-exchange endpoint (server-side).
pub const ANILIST_TOKEN_URL: &str = "https://anilist.co/api/v2/oauth/token";

/// MyAnimeList client id (32-byte hex, public). App Type "Other" —
/// no client_secret is issued; PKCE handles client authentication.
pub const MAL_CLIENT_ID: &str = "30b46eba161e3f6d6f1c89fec89ee683";

/// MAL registered redirect URI (also `ani-gui://` is registered but
/// loopback is the canonical path).
pub const MAL_REDIRECT_URI: &str = "http://localhost:53682/callback";

/// MAL API base.
pub const MAL_API: &str = "https://api.myanimelist.net/v2";

/// MAL OAuth authorize URL.
pub const MAL_AUTH_URL: &str = "https://myanimelist.net/v1/oauth2/authorize";

/// MAL OAuth token-exchange endpoint.
pub const MAL_TOKEN_URL: &str = "https://myanimelist.net/v1/oauth2/token";

/// Fixed port the Electron OAuth callback server binds. Pinned because
/// AniList only allows one redirect URI per client.
pub const OAUTH_CALLBACK_PORT: u16 = 53682;
