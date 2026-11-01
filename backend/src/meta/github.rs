//! GitHub releases lookup for the in-app update notifier.
//!
//! The renderer used to fetch `api.github.com` directly, which
//! broke the documented "backend owns outbound HTTP" boundary
//! (see `docs/architecture.md`). This module is the localhost-side
//! proxy: same shape the renderer used to compose itself, but the
//! external call is centralised here.
//!
//! Endpoint choice:
//! - `include_prereleases=true` → `/releases?per_page=1`. Returns
//!   a JSON array of one element (newest non-draft release,
//!   pre-releases included).
//! - `include_prereleases=false` → `/releases/latest`. Returns a
//!   single object — newest *full* release (prerelease=false,
//!   draft=false). 404 when the repo has no full releases yet,
//!   which is the case for ani-gui today.
//!
//! Failures (network down, GitHub rate limit, malformed payload)
//! collapse to `Ok(None)` so the renderer can branch on a single
//! nullable. We don't propagate transport errors as `AniError` —
//! the update notifier is best-effort by design.

use serde::{Deserialize, Serialize};

const GITHUB_API: &str = "https://api.github.com";

/// Default repo the notifier targets. Kept compile-time constant
/// rather than read from config so a misconfigured runtime can't
/// silently silence the notifier.
pub const REPO_PATH: &str = "JoaoPucci/ani-gui";

/// A platform the app is packaged for. A release is offered to an
/// install only when it carries that install's package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// 64-bit Windows, packaged as the NSIS installer.
    Windows,
    /// 64-bit Linux, packaged as an AppImage and a `.deb`.
    Linux,
}

impl Platform {
    /// The packaged platform this binary was built for, or `None`
    /// on one the app does not package.
    pub fn current() -> Option<Self> {
        None
    }

    /// Whether a release asset named `asset_name` installs on this
    /// platform.
    pub fn carries(self, asset_name: &str) -> bool {
        let suffixes: &[&str] = match self {
            Self::Windows => &["-x64-setup.exe"],
            Self::Linux => &["-x86_64.AppImage", "-amd64.deb"],
        };
        suffixes.iter().any(|s| asset_name.ends_with(s))
    }
}

/// Slim shape of the GitHub `release` object — only what the
/// renderer needs. Serialised back to JSON by the HTTP handler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    /// Git tag the release points at — e.g. `v0.4.0`.
    pub tag: String,
    /// Human-readable release name; falls back to `tag` upstream.
    pub name: String,
    /// Browser-facing GitHub URL the dialog's primary CTA opens.
    pub url: String,
    /// ISO-8601 timestamp the release was published.
    pub published_at: String,
    /// Markdown release-notes body (rendered as pre-formatted
    /// text in the dialog — no Markdown dependency).
    pub body: String,
}

/// Raw shape from the GitHub API. `serde` parses what we need;
/// extra fields are ignored.
#[derive(Debug, Deserialize)]
struct RawRelease {
    #[serde(default)]
    tag_name: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

/// One downloadable file attached to a release.
#[derive(Debug, Deserialize)]
struct RawAsset {
    #[serde(default)]
    name: String,
}

/// Normalise a `RawRelease` into the smaller `ReleaseInfo` or
/// `None` when load-bearing fields are missing. Drafts are
/// filtered (badge would link to a release the user can't see).
fn normalise(raw: RawRelease) -> Option<ReleaseInfo> {
    if raw.draft {
        return None;
    }
    let tag = raw.tag_name?;
    let url = raw.html_url?;
    Some(ReleaseInfo {
        tag: tag.clone(),
        name: raw.name.unwrap_or(tag),
        url,
        published_at: raw.published_at.unwrap_or_default(),
        body: raw.body.unwrap_or_default(),
    })
}

/// The release to offer from `releases`, newest first.
fn select_release(
    releases: Vec<RawRelease>,
    platform: Platform,
    include_prereleases: bool,
) -> Option<ReleaseInfo> {
    let _ = (platform, include_prereleases);
    normalise(releases.into_iter().next()?)
}

/// Parse the list endpoint's body and pick the release to offer.
fn parse_response(
    body: &[u8],
    platform: Platform,
    include_prereleases: bool,
) -> Option<ReleaseInfo> {
    let releases: Vec<RawRelease> = serde_json::from_slice(body).ok()?;
    select_release(releases, platform, include_prereleases)
}

/// Fetch the latest GitHub release for the bundled repo. Returns
/// `Ok(None)` on every soft failure (offline, rate-limited, 404,
/// malformed JSON, missing fields, draft-only). The caller treats
/// `None` and "no newer version" identically — the badge stays
/// hidden either way.
pub async fn fetch_latest_release(
    client: &reqwest::Client,
    include_prereleases: bool,
    platform: Option<Platform>,
    base_override: Option<&str>,
) -> Option<ReleaseInfo> {
    let platform = platform.unwrap_or(Platform::Linux);
    let base = base_override.unwrap_or(GITHUB_API);
    let url = if include_prereleases {
        format!("{base}/repos/{REPO_PATH}/releases?per_page=1")
    } else {
        format!("{base}/repos/{REPO_PATH}/releases/latest")
    };
    let resp = client
        .get(&url)
        // GitHub requires a User-Agent header on every request; an
        // empty UA gets a 403. Using the app name is the
        // GitHub-recommended convention.
        .header(reqwest::header::USER_AGENT, "ani-gui")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    parse_response(&bytes, platform, include_prereleases)
}

#[cfg(test)]
#[path = "github_test.rs"]
mod tests;
