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
