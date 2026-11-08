//! Tests for the update notifier's GitHub releases lookup. Sibling
//! file loaded via `#[path]`; the `_test.rs` suffix matches
//! `tools/crap-score.mjs`'s filter, so these tests stay out of the
//! production file's CRAP roll-up. Moving them back inline puts that
//! file's score at the mercy of its test count.

use super::*;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The files a full release carries, named as the releases name them.
fn every_package(version: &str) -> Vec<String> {
    vec![
        format!("ani-gui-{version}-amd64.deb"),
        format!("ani-gui-{version}-x64-setup.exe"),
        format!("ani-gui-{version}-x86_64.AppImage"),
    ]
}

/// The files a release cut without the Windows installer carries.
fn linux_packages(version: &str) -> Vec<String> {
    vec![
        format!("ani-gui-{version}-amd64.deb"),
        format!("ani-gui-{version}-x86_64.AppImage"),
    ]
}

/// The files a Windows-only release carries.
fn windows_package(version: &str) -> Vec<String> {
    vec![format!("ani-gui-{version}-x64-setup.exe")]
}

fn release_json(version: &str, prerelease: bool, assets: &[String]) -> serde_json::Value {
    serde_json::json!({
        "tag_name": format!("v{version}"),
        "name": format!("v{version} — notes"),
        "html_url": format!("https://github.com/JoaoPucci/ani-gui/releases/tag/v{version}"),
        "published_at": "2026-06-01T00:00:00Z",
        "body": "release notes",
        "draft": false,
        "prerelease": prerelease,
        "assets": assets.iter().map(|n| serde_json::json!({ "name": n })).collect::<Vec<_>>(),
    })
}

fn valid_release_json() -> serde_json::Value {
    release_json("0.5.0", true, &every_package("0.5.0"))
}

fn list(releases: &[serde_json::Value]) -> Vec<u8> {
    serde_json::to_vec(&serde_json::Value::Array(releases.to_vec())).unwrap()
}

/// Parse a list body and pick from it alone, as the lookup does with
/// its page.
fn parse_response(
    body: &[u8],
    platform: Platform,
    include_prereleases: bool,
) -> Option<ReleaseInfo> {
    let releases: Vec<RawRelease> = serde_json::from_slice(body).ok()?;
    select_release(releases, platform, include_prereleases)
}

fn tag_of(body: &[u8], platform: Platform, include_prereleases: bool) -> Option<String> {
    parse_response(body, platform, include_prereleases).map(|r| r.tag)
}

#[test]
fn parse_normalises_the_offered_release() {
    let release =
        parse_response(&list(&[valid_release_json()]), Platform::Linux, true).expect("release");
    assert_eq!(release.tag, "v0.5.0");
    assert_eq!(release.name, "v0.5.0 — notes");
    assert_eq!(release.body, "release notes");
    assert_eq!(
        release.url,
        "https://github.com/JoaoPucci/ani-gui/releases/tag/v0.5.0"
    );
}

#[test]
fn drafts_are_never_offered() {
    // A full release, so the pre-release filter cannot be what drops
    // it when pre-releases are excluded.
    let mut draft = release_json("0.5.0", false, &every_package("0.5.0"));
    draft["draft"] = serde_json::Value::Bool(true);
    for platform in [Platform::Windows, Platform::Linux] {
        for include in [true, false] {
            assert!(tag_of(&list(&[draft.clone()]), platform, include).is_none());
        }
    }
}

#[test]
fn a_release_missing_its_tag_or_url_is_not_offered() {
    let mut no_tag = valid_release_json();
    no_tag.as_object_mut().unwrap().remove("tag_name");
    assert!(tag_of(&list(&[no_tag]), Platform::Linux, true).is_none());

    let mut no_url = valid_release_json();
    no_url.as_object_mut().unwrap().remove("html_url");
    assert!(tag_of(&list(&[no_url]), Platform::Linux, true).is_none());
}

#[test]
fn a_release_missing_its_tag_or_url_is_passed_over_for_the_next() {
    let mut no_tag = release_json("0.6.0", true, &every_package("0.6.0"));
    no_tag.as_object_mut().unwrap().remove("tag_name");
    let mut no_url = release_json("0.5.0", true, &every_package("0.5.0"));
    no_url.as_object_mut().unwrap().remove("html_url");
    let body = list(&[
        no_tag,
        no_url,
        release_json("0.4.0", true, &every_package("0.4.0")),
    ]);
    assert_eq!(
        tag_of(&body, Platform::Linux, true).as_deref(),
        Some("v0.4.0")
    );
}

#[test]
fn empty_list_returns_none() {
    assert!(tag_of(&list(&[]), Platform::Linux, true).is_none());
}

#[test]
fn malformed_json_returns_none() {
    for include in [true, false] {
        assert!(parse_response(b"not-json", Platform::Linux, include).is_none());
    }
}

#[test]
fn name_falls_back_to_tag_when_missing() {
    let mut payload = valid_release_json();
    payload.as_object_mut().unwrap().remove("name");
    let release = parse_response(&list(&[payload]), Platform::Linux, true).expect("release");
    assert_eq!(release.name, "v0.5.0");
}

#[test]
fn a_release_without_assets_is_offered_to_no_platform() {
    let mut bare = valid_release_json();
    bare.as_object_mut().unwrap().remove("assets");
    for platform in [Platform::Windows, Platform::Linux] {
        assert!(tag_of(&list(&[bare.clone()]), platform, true).is_none());
    }
}

#[test]
fn windows_skips_a_newer_release_cut_without_the_installer() {
    let body = list(&[
        release_json("0.18.0", true, &linux_packages("0.18.0")),
        release_json("0.17.0", true, &every_package("0.17.0")),
        release_json("0.16.0", true, &every_package("0.16.0")),
    ]);
    assert_eq!(
        tag_of(&body, Platform::Windows, true).as_deref(),
        Some("v0.17.0")
    );
    assert_eq!(
        tag_of(&body, Platform::Linux, true).as_deref(),
        Some("v0.18.0")
    );
}

#[test]
fn linux_skips_a_newer_windows_only_release() {
    // v0.12.1 shipped the Windows installer alone.
    let body = list(&[
        release_json("0.12.1", true, &windows_package("0.12.1")),
        release_json("0.12.0", true, &every_package("0.12.0")),
    ]);
    assert_eq!(
        tag_of(&body, Platform::Linux, true).as_deref(),
        Some("v0.12.0")
    );
    assert_eq!(
        tag_of(&body, Platform::Windows, true).as_deref(),
        Some("v0.12.1")
    );
}

#[test]
fn excluding_prereleases_skips_them_for_the_newest_full_release() {
    let body = list(&[
        release_json("0.3.0", true, &every_package("0.3.0")),
        release_json("0.2.0", false, &every_package("0.2.0")),
    ]);
    assert_eq!(
        tag_of(&body, Platform::Linux, false).as_deref(),
        Some("v0.2.0")
    );
    assert_eq!(
        tag_of(&body, Platform::Linux, true).as_deref(),
        Some("v0.3.0")
    );
}

#[test]
fn excluding_prereleases_offers_nothing_when_every_release_is_one() {
    let body = list(&[
        release_json("0.3.0", true, &every_package("0.3.0")),
        release_json("0.2.0", true, &every_package("0.2.0")),
    ]);
    assert!(tag_of(&body, Platform::Windows, false).is_none());
    assert!(tag_of(&body, Platform::Linux, false).is_none());
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn a_64_bit_linux_build_is_the_linux_platform() {
    assert_eq!(Platform::current(), Some(Platform::Linux));
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[test]
fn a_64_bit_windows_build_is_the_windows_platform() {
    assert_eq!(Platform::current(), Some(Platform::Windows));
}

/// One generated release: whether it is a pre-release, whether it is
/// a draft, and which of the three packages it carries (bit 0 the
/// installer, bit 1 the AppImage, bit 2 the `.deb`).
type Shape = (bool, bool, u8);

fn raw_from(index: usize, (prerelease, draft, mask): Shape) -> RawRelease {
    let version = format!("0.{index}.0");
    let packages = [
        format!("ani-gui-{version}-x64-setup.exe"),
        format!("ani-gui-{version}-x86_64.AppImage"),
        format!("ani-gui-{version}-amd64.deb"),
    ];
    RawRelease {
        tag_name: Some(format!("v{version}")),
        name: None,
        html_url: Some(format!("https://example.invalid/v{version}")),
        published_at: None,
        body: None,
        draft,
        prerelease,
        assets: packages
            .into_iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, name)| RawAsset { name })
            .collect(),
    }
}

proptest::proptest! {
    /// The offer is the first release in the list that is published,
    /// counts under the pre-release setting, and carries a package for
    /// the platform — and nothing when no release is all three.
    #[test]
    fn the_offer_is_the_newest_release_installable_here(
        shapes in proptest::collection::vec((proptest::bool::ANY, proptest::bool::ANY, 0u8..8), 0..12),
        windows in proptest::bool::ANY,
        include_prereleases in proptest::bool::ANY,
    ) {
        let platform = if windows { Platform::Windows } else { Platform::Linux };
        let installable = |&(prerelease, draft, mask): &Shape| {
            let carried = if windows { mask & 0b001 != 0 } else { mask & 0b110 != 0 };
            !draft && (include_prereleases || !prerelease) && carried
        };
        let want = shapes.iter().position(installable).map(|i| format!("v0.{i}.0"));
        let releases = shapes.iter().enumerate().map(|(i, s)| raw_from(i, *s)).collect();
        let got = select_release(releases, platform, include_prereleases).map(|r| r.tag);
        proptest::prop_assert_eq!(got, want);
    }
}

#[tokio::test]
async fn the_walk_reads_a_bounded_page_of_recent_releases_for_either_setting() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO_PATH}/releases")))
        .and(query_param("per_page", "10"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([release_json(
                "0.2.0",
                false,
                &every_package("0.2.0")
            )])),
        )
        .expect(2)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    for include in [true, false] {
        let release =
            fetch_latest_release(&client, include, Some(Platform::Linux), Some(&server.uri()))
                .await;
        assert_eq!(release.map(|r| r.tag).as_deref(), Some("v0.2.0"));
    }
}

/// Mounts the list endpoint answering with `releases`.
async fn serving(releases: &[serde_json::Value]) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO_PATH}/releases")))
        .and(query_param("per_page", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(releases.to_vec()))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn a_windows_install_is_offered_the_newest_release_with_an_installer() {
    let server = serving(&[
        release_json("0.18.0", true, &linux_packages("0.18.0")),
        release_json("0.17.0", true, &every_package("0.17.0")),
    ])
    .await;
    let client = reqwest::Client::new();
    let release =
        fetch_latest_release(&client, true, Some(Platform::Windows), Some(&server.uri())).await;
    assert_eq!(release.map(|r| r.tag).as_deref(), Some("v0.17.0"));
}

#[tokio::test]
async fn a_linux_install_is_offered_the_newest_release_with_a_linux_package() {
    let server = serving(&[
        release_json("0.12.1", true, &windows_package("0.12.1")),
        release_json("0.12.0", true, &every_package("0.12.0")),
    ])
    .await;
    let client = reqwest::Client::new();
    let release =
        fetch_latest_release(&client, true, Some(Platform::Linux), Some(&server.uri())).await;
    assert_eq!(release.map(|r| r.tag).as_deref(), Some("v0.12.0"));
}

#[tokio::test]
async fn an_unpackaged_platform_is_offered_nothing_and_asks_github_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([valid_release_json()])),
        )
        .expect(0)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    assert!(
        fetch_latest_release(&client, true, None, Some(&server.uri()))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn fetch_returns_none_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    for include in [true, false] {
        assert!(
            fetch_latest_release(&client, include, Some(Platform::Linux), Some(&server.uri()))
                .await
                .is_none()
        );
    }
}

// Asset names as the releases carry them, e.g. v0.16.0:
// ani-gui-0.16.0-x64-setup.exe, ani-gui-0.16.0-x86_64.AppImage,
// ani-gui-0.16.0-amd64.deb.

#[test]
fn windows_carries_the_nsis_installer_and_nothing_linux_ships() {
    assert!(Platform::Windows.carries("ani-gui-0.16.0-x64-setup.exe"));
    assert!(!Platform::Windows.carries("ani-gui-0.16.0-x86_64.AppImage"));
    assert!(!Platform::Windows.carries("ani-gui-0.16.0-amd64.deb"));
}

#[test]
fn linux_carries_the_appimage_or_the_deb_and_not_the_installer() {
    assert!(Platform::Linux.carries("ani-gui-0.16.0-x86_64.AppImage"));
    assert!(Platform::Linux.carries("ani-gui-0.16.0-amd64.deb"));
    assert!(!Platform::Linux.carries("ani-gui-0.16.0-x64-setup.exe"));
}

#[test]
fn neither_platform_carries_an_unrelated_file() {
    for name in ["SHA256SUMS", "ani-gui-0.16.0.tar.gz", "latest.yml", ""] {
        assert!(!Platform::Windows.carries(name), "{name}");
        assert!(!Platform::Linux.carries(name), "{name}");
    }
}

/// The endings the lookup accepts are the ones in
/// `release_assets.json`, the file `electron/lib/release-asset-names.cjs`
/// checks electron-builder's file names against. A package renamed on
/// one side only fails that check instead of silencing every update.
#[test]
fn the_lookup_accepts_exactly_the_endings_the_packaging_check_reads() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/meta/release_assets.json");
    let text = std::fs::read_to_string(path).expect("the endings file");
    let file: serde_json::Value = serde_json::from_str(&text).expect("the endings file is JSON");
    for (platform, key, other) in [
        (Platform::Windows, "windows", Platform::Linux),
        (Platform::Linux, "linux", Platform::Windows),
    ] {
        let mut listed: Vec<String> = file[key]
            .as_object()
            .expect(key)
            .values()
            .map(|v| v.as_str().expect("an ending is a string").to_owned())
            .collect();
        listed.sort();
        let mut accepted = platform.endings();
        accepted.sort();
        assert_eq!(accepted, listed, "{key}");
        for ending in &listed {
            let name = format!("ani-gui-9.9.9{ending}");
            assert!(platform.carries(&name), "{name}");
            assert!(!other.carries(&name), "{name}");
        }
    }
}

proptest::proptest! {
    /// Whatever the version in the name, each platform recognises its
    /// own packages and never the other platform's.
    #[test]
    fn each_platform_carries_its_own_packages_and_never_the_others(
        version in "[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}(-[a-z0-9.]{1,8})?",
    ) {
        let exe = format!("ani-gui-{version}-x64-setup.exe");
        let appimage = format!("ani-gui-{version}-x86_64.AppImage");
        let deb = format!("ani-gui-{version}-amd64.deb");
        proptest::prop_assert!(Platform::Windows.carries(&exe));
        proptest::prop_assert!(!Platform::Windows.carries(&appimage));
        proptest::prop_assert!(!Platform::Windows.carries(&deb));
        proptest::prop_assert!(Platform::Linux.carries(&appimage));
        proptest::prop_assert!(Platform::Linux.carries(&deb));
        proptest::prop_assert!(!Platform::Linux.carries(&exe));
    }

    /// A name ending in none of the package suffixes installs nowhere.
    #[test]
    fn a_name_without_a_package_suffix_is_carried_by_neither(name in "\\PC{0,40}") {
        let packaged = ["-x64-setup.exe", "-x86_64.AppImage", "-amd64.deb"]
            .iter()
            .any(|s| name.ends_with(s));
        proptest::prop_assume!(!packaged);
        proptest::prop_assert!(!Platform::Windows.carries(&name));
        proptest::prop_assert!(!Platform::Linux.carries(&name));
    }
}

/// Ten pre-releases: a full page that, with pre-releases excluded,
/// holds nothing to offer.
fn ten_prereleases() -> Vec<serde_json::Value> {
    (11..21)
        .rev()
        .map(|minor| {
            let version = format!("0.{minor}.0");
            release_json(&version, true, &every_package(&version))
        })
        .collect()
}

/// Mounts `/releases/latest` answering with `release`, expected to be
/// asked `times` times.
async fn latest_serving(server: &MockServer, release: serde_json::Value, times: u64) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO_PATH}/releases/latest")))
        .respond_with(ResponseTemplate::new(200).set_body_json(release))
        .expect(times)
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_full_release_older_than_the_page_is_still_found() {
    let server = serving(&ten_prereleases()).await;
    latest_serving(
        &server,
        release_json("0.10.0", false, &every_package("0.10.0")),
        2,
    )
    .await;
    let client = reqwest::Client::new();
    for platform in [Platform::Windows, Platform::Linux] {
        let release =
            fetch_latest_release(&client, false, Some(platform), Some(&server.uri())).await;
        assert_eq!(release.map(|r| r.tag).as_deref(), Some("v0.10.0"));
    }
}

#[tokio::test]
async fn an_older_full_release_without_this_platforms_package_is_not_offered() {
    let server = serving(&ten_prereleases()).await;
    latest_serving(
        &server,
        release_json("0.10.0", false, &linux_packages("0.10.0")),
        1,
    )
    .await;
    let client = reqwest::Client::new();
    let release =
        fetch_latest_release(&client, false, Some(Platform::Windows), Some(&server.uri())).await;
    assert!(release.is_none());
}

#[tokio::test]
async fn an_older_draft_or_pre_release_from_the_fallback_is_not_offered() {
    let mut draft = release_json("0.10.0", false, &every_package("0.10.0"));
    draft["draft"] = serde_json::Value::Bool(true);
    // A page with nothing installable on Linux under either setting,
    // so the fallback is asked whether pre-releases count or not: it
    // offers a full release only.
    let page: Vec<_> = (11..21)
        .rev()
        .map(|minor| {
            let version = format!("0.{minor}.0");
            release_json(&version, true, &windows_package(&version))
        })
        .collect();
    for include in [true, false] {
        for stale in [
            draft.clone(),
            release_json("0.10.0", true, &every_package("0.10.0")),
        ] {
            let server = serving(&page).await;
            latest_serving(&server, stale, 1).await;
            let client = reqwest::Client::new();
            let release =
                fetch_latest_release(&client, include, Some(Platform::Linux), Some(&server.uri()))
                    .await;
            assert!(release.is_none());
        }
    }
}

#[tokio::test]
async fn an_offer_on_the_page_asks_for_nothing_more() {
    let server = serving(&[release_json("0.21.0", false, &every_package("0.21.0"))]).await;
    latest_serving(
        &server,
        release_json("0.10.0", false, &every_package("0.10.0")),
        0,
    )
    .await;
    let client = reqwest::Client::new();
    for include in [true, false] {
        let release =
            fetch_latest_release(&client, include, Some(Platform::Linux), Some(&server.uri()))
                .await;
        assert_eq!(release.map(|r| r.tag).as_deref(), Some("v0.21.0"));
    }
}

#[tokio::test]
async fn a_page_that_fails_to_load_asks_for_nothing_more() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO_PATH}/releases")))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    latest_serving(
        &server,
        release_json("0.10.0", false, &every_package("0.10.0")),
        0,
    )
    .await;
    let client = reqwest::Client::new();
    let release =
        fetch_latest_release(&client, false, Some(Platform::Linux), Some(&server.uri())).await;
    assert!(release.is_none());
}

#[tokio::test]
async fn an_older_full_release_is_found_when_no_pre_release_on_the_page_installs_here() {
    let page: Vec<_> = (11..21)
        .rev()
        .map(|minor| {
            let version = format!("0.{minor}.0");
            release_json(&version, true, &linux_packages(&version))
        })
        .collect();
    let server = serving(&page).await;
    latest_serving(
        &server,
        release_json("0.10.0", false, &every_package("0.10.0")),
        1,
    )
    .await;
    let client = reqwest::Client::new();
    let release =
        fetch_latest_release(&client, true, Some(Platform::Windows), Some(&server.uri())).await;
    assert_eq!(release.map(|r| r.tag).as_deref(), Some("v0.10.0"));
}

#[tokio::test]
async fn a_page_github_answers_with_something_unreadable_falls_back_to_the_latest_full_release() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO_PATH}/releases")))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
        .mount(&server)
        .await;
    latest_serving(
        &server,
        release_json("0.10.0", false, &every_package("0.10.0")),
        1,
    )
    .await;
    let client = reqwest::Client::new();
    let release =
        fetch_latest_release(&client, false, Some(Platform::Linux), Some(&server.uri())).await;
    assert_eq!(release.map(|r| r.tag).as_deref(), Some("v0.10.0"));
}

// ---- The naming fallback --------------------------------------------------
//
// The lookup knows a package by how its file name ends. If a release's
// files are ever named some other way, an install could no longer tell
// any release apart from one cut without its platform, and would see
// "no update" for good. So when nothing read carries a file any
// packaged platform recognises, the lookup takes the naming to have
// changed and offers the newest release by version alone, as it did
// before it read files at all.

/// A release whose files are named in a way no platform recognises.
fn renamed(version: &str, prerelease: bool) -> RawRelease {
    raw(
        version,
        prerelease,
        false,
        &[
            format!("ani-gui-{version}-x86-64.AppImage"),
            format!("ani-gui-{version}-setup-x64.exe"),
        ],
    )
}

fn raw(version: &str, prerelease: bool, draft: bool, assets: &[String]) -> RawRelease {
    RawRelease {
        tag_name: Some(format!("v{version}")),
        name: None,
        html_url: Some(format!("https://example.invalid/v{version}")),
        published_at: None,
        body: None,
        draft,
        prerelease,
        assets: assets
            .iter()
            .map(|name| RawAsset { name: name.clone() })
            .collect(),
    }
}

fn offered(
    page: Vec<RawRelease>,
    latest: Option<RawRelease>,
    platform: Platform,
    include_prereleases: bool,
) -> Option<String> {
    offer(page, latest, platform, include_prereleases).map(|r| r.tag)
}

#[test]
fn files_no_platform_recognises_offer_the_newest_release_by_version() {
    let page = vec![renamed("0.19.0", true), renamed("0.18.0", false)];
    for platform in [Platform::Windows, Platform::Linux] {
        assert_eq!(
            offered(page.clone(), None, platform, true).as_deref(),
            Some("v0.19.0")
        );
        // With pre-releases off the check used to ask
        // `/releases/latest` and nothing else; the fallback does the
        // same, so with no latest release read it offers nothing.
        assert_eq!(offered(page.clone(), None, platform, false), None);
    }
}

#[test]
fn with_pre_releases_off_the_fallback_offers_the_latest_full_release_as_before() {
    // A full release on the page newer than the one GitHub marks latest
    // (published with "make latest" off) is not what the check offered
    // before it read files, and is not what the fallback offers.
    let page = vec![renamed("0.19.0", false)];
    for platform in [Platform::Windows, Platform::Linux] {
        assert_eq!(
            offered(
                page.clone(),
                Some(renamed("0.18.0", false)),
                platform,
                false
            )
            .as_deref(),
            Some("v0.18.0")
        );
    }
}

#[test]
fn the_fallback_never_offers_a_release_without_files() {
    // A release whose files have not finished uploading has nothing to
    // download; the fallback passes over it for the next.
    let page = vec![raw("0.20.0", true, false, &[]), renamed("0.19.0", true)];
    assert_eq!(
        offered(page, None, Platform::Linux, true).as_deref(),
        Some("v0.19.0")
    );
    let page = vec![renamed("0.19.0", true)];
    assert_eq!(
        offered(
            page,
            Some(raw("0.18.0", false, false, &[])),
            Platform::Linux,
            false
        ),
        None
    );
}

#[test]
fn the_naming_fallback_keeps_the_draft_and_pre_release_rules() {
    let mut draft = renamed("0.20.0", false);
    draft.draft = true;
    let page = vec![draft, renamed("0.19.0", true)];
    assert_eq!(
        offered(page.clone(), None, Platform::Linux, true).as_deref(),
        Some("v0.19.0")
    );
    // Excluding pre-releases, the page holds nothing; the newest full
    // release comes from `/releases/latest`, which must be full itself.
    assert_eq!(offered(page.clone(), None, Platform::Linux, false), None);
    assert_eq!(
        offered(
            page.clone(),
            Some(renamed("0.17.0", false)),
            Platform::Linux,
            false
        )
        .as_deref(),
        Some("v0.17.0")
    );
    assert_eq!(
        offered(page, Some(renamed("0.17.0", true)), Platform::Linux, false),
        None
    );
}

#[test]
fn the_latest_full_releases_files_count_toward_what_was_read() {
    // A page of renamed pre-releases, and a latest full release whose
    // files Linux recognises: the naming has not changed, Windows was
    // simply skipped, and gets nothing.
    let page = vec![renamed("0.19.0", true)];
    let latest = raw("0.18.0", false, false, &linux_packages("0.18.0"));
    assert_eq!(
        offered(page.clone(), Some(latest.clone()), Platform::Windows, false),
        None
    );
    assert_eq!(
        offered(page, Some(latest), Platform::Linux, false).as_deref(),
        Some("v0.18.0")
    );
}

#[test]
fn a_release_with_another_platforms_packages_only_is_still_skipped() {
    // v0.12.1 shipped the Windows installer alone: recognisable files,
    // so a Linux install skips it rather than falling back.
    let page = vec![raw("0.12.1", true, false, &windows_package("0.12.1"))];
    assert_eq!(offered(page.clone(), None, Platform::Linux, true), None);
    let with_older = vec![
        raw("0.12.1", true, false, &windows_package("0.12.1")),
        raw("0.12.0", true, false, &every_package("0.12.0")),
    ];
    assert_eq!(
        offered(with_older, None, Platform::Linux, true).as_deref(),
        Some("v0.12.0")
    );
    // One recognisable file anywhere in what was read is enough.
    let mixed = vec![
        renamed("0.13.0", true),
        raw("0.12.1", true, false, &windows_package("0.12.1")),
    ];
    assert_eq!(offered(mixed, None, Platform::Linux, true), None);
}

#[test]
fn releases_with_no_files_at_all_do_not_count_as_renamed() {
    // A release published before its files finish uploading carries
    // none. That says nothing about how files are named, and offering
    // it would send the install to a page with nothing to download.
    let page = vec![
        raw("0.19.0", true, false, &[]),
        raw("0.18.0", false, false, &[]),
    ];
    for include in [true, false] {
        assert_eq!(offered(page.clone(), None, Platform::Linux, include), None);
    }
}

#[tokio::test]
async fn the_lookup_falls_back_when_every_file_it_read_is_named_some_other_way() {
    let page: Vec<_> = ["0.19.0", "0.18.0"]
        .iter()
        .map(|v| {
            release_json(
                v,
                true,
                &[
                    format!("ani-gui-{v}-x86-64.AppImage"),
                    format!("ani-gui-{v}-setup-x64.exe"),
                ],
            )
        })
        .collect();
    let server = serving(&page).await;
    latest_serving(
        &server,
        release_json(
            "0.17.0",
            false,
            &[String::from("ani-gui-0.17.0-x86-64.AppImage")],
        ),
        2,
    )
    .await;
    let client = reqwest::Client::new();
    let with_pre =
        fetch_latest_release(&client, true, Some(Platform::Windows), Some(&server.uri())).await;
    assert_eq!(with_pre.map(|r| r.tag).as_deref(), Some("v0.19.0"));
    let full =
        fetch_latest_release(&client, false, Some(Platform::Linux), Some(&server.uri())).await;
    assert_eq!(full.map(|r| r.tag).as_deref(), Some("v0.17.0"));
}

#[tokio::test]
async fn the_fallback_reads_the_page_alone_when_the_latest_release_does_not_answer() {
    let v = "0.19.0";
    let server = serving(&[release_json(
        v,
        true,
        &[format!("ani-gui-{v}-x86-64.AppImage")],
    )])
    .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO_PATH}/releases/latest")))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let release =
        fetch_latest_release(&client, true, Some(Platform::Linux), Some(&server.uri())).await;
    assert_eq!(release.map(|r| r.tag).as_deref(), Some("v0.19.0"));
}

/// One generated release for the full oracle: pre-release, draft,
/// which of the three packages it carries (as in [`Shape`]), and
/// whether it also carries a file named in a way no platform knows.
type FullShape = (bool, bool, u8, bool);

fn raw_full(version: &str, (prerelease, draft, mask, unknown): FullShape) -> RawRelease {
    let packages = [
        format!("ani-gui-{version}-x64-setup.exe"),
        format!("ani-gui-{version}-x86_64.AppImage"),
        format!("ani-gui-{version}-amd64.deb"),
    ];
    let mut assets: Vec<String> = packages
        .into_iter()
        .enumerate()
        .filter(|(bit, _)| mask & (1 << bit) != 0)
        .map(|(_, name)| name)
        .collect();
    if unknown {
        assets.push(format!("ani-gui-{version}-x86-64.AppImage"));
    }
    raw(version, prerelease, draft, &assets)
}

/// Weighted toward releases carrying no package, so that pages where
/// nothing is recognised — the fallback's case — come up often.
fn full_shape() -> impl proptest::strategy::Strategy<Value = FullShape> {
    use proptest::strategy::{Just, Strategy};
    let mask = proptest::prop_oneof![3 => Just(0u8), 1 => 0u8..8].boxed();
    (
        proptest::bool::ANY,
        proptest::bool::ANY,
        mask,
        proptest::bool::ANY,
    )
}

proptest::proptest! {
    /// The whole choice, page and latest together: the newest page
    /// release installable here; else the latest release, if it is a
    /// published full release carrying this platform's package; else,
    /// when something read carries files and none of them is any
    /// platform's package, what the check offered before it read files:
    /// with pre-releases on, the newest published page release that
    /// carries files, or else the latest release; with them off, the
    /// latest release; the latest only if it is a published full release
    /// carrying files. Else nothing.
    #[test]
    fn the_offer_follows_the_platform_then_falls_back_only_on_unknown_naming(
        page in proptest::collection::vec(full_shape(), 0..6),
        latest in proptest::option::of(full_shape()),
        windows in proptest::bool::ANY,
        include_prereleases in proptest::bool::ANY,
    ) {
        let platform = if windows { Platform::Windows } else { Platform::Linux };
        let carried = |mask: u8| if windows { mask & 0b001 != 0 } else { mask & 0b110 != 0 };
        let eligible = |&(pre, draft, _, _): &FullShape, inc: bool| !draft && (inc || !pre);
        let latest_tag = "v9.9.0".to_owned();
        let page_tag = |i: usize| format!("v0.{i}.0");
        let read: Vec<FullShape> = page.iter().copied().chain(latest).collect();
        let any_file = read.iter().any(|&(_, _, mask, unknown)| mask != 0 || unknown);
        let recognised = read.iter().any(|&(_, _, mask, _)| mask != 0);

        let want = page
            .iter()
            .position(|s| eligible(s, include_prereleases) && carried(s.2))
            .map(page_tag)
            .or_else(|| latest.filter(|s| eligible(s, false) && carried(s.2)).map(|_| latest_tag.clone()))
            .or_else(|| {
                let has_files = |&(_, _, mask, unknown): &FullShape| mask != 0 || unknown;
                let latest_full = || {
                    latest
                        .filter(|s| eligible(s, false) && has_files(s))
                        .map(|_| latest_tag.clone())
                };
                (any_file && !recognised).then(|| {
                    if include_prereleases {
                        page.iter()
                            .position(|s| eligible(s, true) && has_files(s))
                            .map(page_tag)
                            .or_else(latest_full)
                    } else {
                        latest_full()
                    }
                })?
            });

        let raws = page.iter().enumerate().map(|(i, s)| raw_full(&format!("0.{i}.0"), *s)).collect();
        let got = offered(raws, latest.map(|s| raw_full("9.9.0", s)), platform, include_prereleases);
        proptest::prop_assert_eq!(got, want);
    }
}
