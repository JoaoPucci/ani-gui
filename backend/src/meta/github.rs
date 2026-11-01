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

/// Parse the response body bytes. The list endpoint returns an
/// array, the latest endpoint a single object — accept both.
fn parse_response(body: &[u8], include_prereleases: bool) -> Option<ReleaseInfo> {
    if include_prereleases {
        let arr: Vec<RawRelease> = serde_json::from_slice(body).ok()?;
        let raw = arr.into_iter().next()?;
        normalise(raw)
    } else {
        let raw: RawRelease = serde_json::from_slice(body).ok()?;
        normalise(raw)
    }
}

/// Fetch the latest GitHub release for the bundled repo. Returns
/// `Ok(None)` on every soft failure (offline, rate-limited, 404,
/// malformed JSON, missing fields, draft-only). The caller treats
/// `None` and "no newer version" identically — the badge stays
/// hidden either way.
pub async fn fetch_latest_release(
    client: &reqwest::Client,
    include_prereleases: bool,
    base_override: Option<&str>,
) -> Option<ReleaseInfo> {
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
    parse_response(&bytes, include_prereleases)
}

#[cfg(test)]
#[path = "github_test.rs"]
mod tests;
