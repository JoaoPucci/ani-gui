//! GitHub releases lookup for the in-app update notifier.
//!
//! The renderer used to fetch `api.github.com` directly, which
//! broke the documented "backend owns outbound HTTP" boundary
//! (see `docs/architecture.md`). This module is the localhost-side
//! proxy: same shape the renderer used to compose itself, but the
//! external call is centralised here.
//!
//! What is offered: the lookup reads the most recent releases, a
//! page of [`RELEASE_PAGE`], newest first, and offers the first one
//! that is published (not a draft), counts under the pre-release
//! setting (with `include_prereleases=false` a pre-release does not),
//! and carries a package for the platform this binary was built for
//! (see [`Platform::carries`]). A release cut without one platform's
//! package is skipped for that platform, which is offered the newest
//! release it can install instead. A platform the app does not
//! package is offered nothing and asks GitHub nothing. The version
//! comparison against the running app stays in the renderer.
//!
//! When the page loads and holds nothing to offer, the lookup also
//! asks `/releases/latest` for the newest full release, however far
//! back it sits, and offers it under the same rules. A pre-release
//! further back than the page reaches is not found.
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

/// How many of the most recent releases the walk reads.
const RELEASE_PAGE: usize = 10;

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
        if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(Self::Windows)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Self::Linux)
        } else {
            None
        }
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

/// The release to offer from `releases`, newest first: the first
/// that counts under the pre-release setting, carries a package for
/// `platform`, and normalises (published, with a tag and a URL).
fn select_release(
    releases: Vec<RawRelease>,
    platform: Platform,
    include_prereleases: bool,
) -> Option<ReleaseInfo> {
    releases
        .into_iter()
        .filter(|r| include_prereleases || !r.prerelease)
        .filter(|r| r.assets.iter().any(|a| platform.carries(&a.name)))
        .find_map(normalise)
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

/// Fetch the newest release `platform` can install. Returns `None`
/// for a platform the app does not package, and on every soft
/// failure (offline, rate-limited, 404, malformed JSON, no release
/// that qualifies). The caller treats `None` and "no newer version"
/// identically — the badge stays hidden either way.
pub async fn fetch_latest_release(
    client: &reqwest::Client,
    include_prereleases: bool,
    platform: Option<Platform>,
    base_override: Option<&str>,
) -> Option<ReleaseInfo> {
    let platform = platform?;
    let base = base_override.unwrap_or(GITHUB_API);
    let page = get_body(
        client,
        &format!("{base}/repos/{REPO_PATH}/releases?per_page={RELEASE_PAGE}"),
    )
    .await?;
    if let Some(offer) = parse_response(&page, platform, include_prereleases) {
        return Some(offer);
    }
    let latest = get_body(client, &format!("{base}/repos/{REPO_PATH}/releases/latest")).await?;
    let raw: RawRelease = serde_json::from_slice(&latest).ok()?;
    select_release(vec![raw], platform, false)
}

/// GET `url` from the GitHub API and return the body of a successful
/// answer.
async fn get_body(client: &reqwest::Client, url: &str) -> Option<bytes::Bytes> {
    let resp = client
        .get(url)
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
    resp.bytes().await.ok()
}

#[cfg(test)]
#[path = "github_test.rs"]
mod tests;
