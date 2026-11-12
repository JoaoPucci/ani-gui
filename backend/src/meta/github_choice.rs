//! The choice of release to offer, from the releases the update check
//! read (see the parent module's docs for what is offered and why).
//! Split from `github.rs` so each file stays under the CRAP gate's
//! high-risk bar.

use serde::Deserialize;

use super::{Platform, ReleaseInfo};

/// Raw shape from the GitHub API. `serde` parses what we need;
/// extra fields are ignored.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct RawRelease {
    #[serde(default)]
    pub(super) tag_name: Option<String>,
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) html_url: Option<String>,
    #[serde(default)]
    pub(super) published_at: Option<String>,
    #[serde(default)]
    pub(super) body: Option<String>,
    #[serde(default)]
    pub(super) draft: bool,
    #[serde(default)]
    pub(super) prerelease: bool,
    #[serde(default)]
    pub(super) assets: Vec<RawAsset>,
}

/// One downloadable file attached to a release.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct RawAsset {
    #[serde(default)]
    pub(super) name: String,
}

/// Normalise a `RawRelease` into the smaller `ReleaseInfo` or
/// `None` when load-bearing fields are missing or the release page
/// is not an https URL. Drafts are filtered (badge would link to a
/// release the user can't see).
fn normalise(raw: RawRelease) -> Option<ReleaseInfo> {
    if raw.draft {
        return None;
    }
    let tag = raw.tag_name?;
    let url = raw.html_url.filter(|u| is_https_page(u))?;
    Some(ReleaseInfo {
        tag: tag.clone(),
        name: raw.name.unwrap_or(tag),
        url,
        published_at: raw.published_at.unwrap_or_default(),
        body: raw.body.unwrap_or_default(),
    })
}

/// Whether `url` parses as an https URL. The update dialog links to
/// the release's page, so anything else is not offered.
fn is_https_page(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| u.scheme() == "https")
}

/// The release to offer from `releases`, newest first: the first
/// that counts under the pre-release setting, carries a package for
/// `platform`, and normalises (published, with a tag and a URL).
pub(super) fn select_release(
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
/// chosen as before files were read ([`offer_as_before_files`]), so a
/// renamed package cannot silence every update.
pub(super) fn offer(
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
         offering the release the check offered before it read files"
    );
    offer_as_before_files(page, latest, include_prereleases)
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
/// page release, or the latest full release when none on the page has
/// files; without, the latest full release alone, as the
/// `/releases/latest` request it made then. A release carrying no
/// files has nothing to download and is passed over either way.
fn offer_as_before_files(
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
