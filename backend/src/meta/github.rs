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
//! (see [`Platform::carries`]; the endings are in `release_assets.json`,
//! which `electron/lib/release-asset-names.cjs` holds to the names
//! electron-builder gives the packages). A release cut without one platform's
//! package is skipped for that platform, which is offered the newest
//! release it can install instead. A platform the app does not
//! package is offered nothing and asks GitHub nothing. The version
//! comparison against the running app stays in the renderer.
//!
//! When the page arrives but offers nothing — no release on it
//! qualifies, or its body cannot be read as a list of releases — the
//! lookup also asks `/releases/latest` for the newest full release,
//! however far back it sits, and offers it if it is published, a
//! full release, and carries this platform's package, whatever the
//! pre-release setting. A page that fails to arrive (network error,
//! non-success status) ends the lookup. A pre-release further back
//! than the page reaches is not found.
//!
//! A failsafe for the naming: when nothing offers and the releases
//! read carry files, none of which any packaged platform recognises,
//! the lookup takes the naming to have changed and offers what it
//! offered before it read files, with a warning in the log: with
//! pre-releases counted, the newest published release on the page;
//! without, the latest full release. A release with no files is never
//! offered this way, and releases with no files at all do not trigger
//! it. A release whose recognisable files are another platform's is
//! still skipped.
//!
//! Failures (network down, GitHub rate limit, malformed payload)
//! collapse to `None` so the renderer can branch on a single
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
    /// Every platform the app is packaged for.
    pub const ALL: [Self; 2] = [Self::Windows, Self::Linux];

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

    /// How the names of this platform's packages end, as
    /// `release_assets.json` lists them.
    pub fn endings(self) -> Vec<String> {
        let file = release_assets();
        let targets = match self {
            Self::Windows => &file.windows,
            Self::Linux => &file.linux,
        };
        targets.values().cloned().collect()
    }

    /// Whether a release asset named `asset_name` installs on this
    /// platform.
    pub fn carries(self, asset_name: &str) -> bool {
        self.endings()
            .iter()
            .any(|s| asset_name.ends_with(s.as_str()))
    }
}

/// The packages' name endings, per platform and electron-builder
/// target. The same file is what `electron/lib/release-asset-names.cjs`
/// checks electron-builder's real file names against, so the endings
/// and the packaging cannot drift apart without that check failing.
#[derive(Debug, Deserialize)]
struct ReleaseAssets {
    linux: std::collections::BTreeMap<String, String>,
    windows: std::collections::BTreeMap<String, String>,
}

fn release_assets() -> &'static ReleaseAssets {
    static FILE: std::sync::OnceLock<ReleaseAssets> = std::sync::OnceLock::new();
    FILE.get_or_init(|| {
        // Embedded at compile time; the tests read it, so a malformed
        // file fails them rather than reaching a release.
        serde_json::from_str(include_str!("release_assets.json"))
            .expect("release_assets.json lists each platform's endings")
    })
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
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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

/// The release to offer from what the lookup read: the page of recent
/// releases, newest first, and — when the page offered nothing and
/// GitHub answered — the newest full release. The page's pick comes
/// first; the latest full release is offered only as a full release
/// carrying this platform's package.
///
/// When neither offers anything and the releases read carry files but
/// none that any packaged platform recognises, the naming has changed
/// rather than this platform having been skipped: the release is
/// chosen as before files were read ([`newest_by_version`]), so a
/// renamed package cannot silence every update.
fn offer(
    page: Vec<RawRelease>,
    latest: Option<RawRelease>,
    platform: Platform,
    include_prereleases: bool,
) -> Option<ReleaseInfo> {
    if let Some(found) = select_release(page.clone(), platform, include_prereleases) {
        return Some(found);
    }
    if let Some(found) = select_release(latest.clone().into_iter().collect(), platform, false) {
        return Some(found);
    }
    if !naming_unrecognised(&page, latest.as_ref()) {
        return None;
    }
    tracing::warn!(
        "update check: no release read carries a file any packaged platform recognises; \
         offering a release by version alone"
    );
    newest_by_version(page, latest, include_prereleases)
}

/// Whether the releases read carry files, and none of them is any
/// packaged platform's package. A release with no files at all says
/// nothing about how files are named.
fn naming_unrecognised(page: &[RawRelease], latest: Option<&RawRelease>) -> bool {
    let assets = || page.iter().chain(latest).flat_map(|r| &r.assets);
    assets().next().is_some() && !assets().any(|a| Platform::ALL.iter().any(|p| p.carries(&a.name)))
}

/// What the check offered before it read files, for when their naming
/// is not recognised: with pre-releases counted, the newest published
/// page release; without, the latest full release alone, as the
/// `/releases/latest` request it made then. A release carrying no
/// files has nothing to download and is passed over either way.
fn newest_by_version(
    page: Vec<RawRelease>,
    latest: Option<RawRelease>,
    include_prereleases: bool,
) -> Option<ReleaseInfo> {
    let has_files = |r: &RawRelease| !r.assets.is_empty();
    let latest_full = || {
        latest
            .filter(|r| !r.prerelease && has_files(r))
            .and_then(normalise)
    };
    if !include_prereleases {
        return latest_full();
    }
    page.into_iter()
        .filter(has_files)
        .find_map(normalise)
        .or_else(latest_full)
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
    let page: Vec<RawRelease> = serde_json::from_slice(&page).unwrap_or_default();
    if let Some(found) = select_release(page.clone(), platform, include_prereleases) {
        return Some(found);
    }
    let latest = get_body(client, &format!("{base}/repos/{REPO_PATH}/releases/latest"))
        .await
        .and_then(|body| serde_json::from_slice::<RawRelease>(&body).ok());
    offer(page, latest, platform, include_prereleases)
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
