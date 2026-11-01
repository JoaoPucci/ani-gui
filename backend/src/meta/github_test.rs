//! Tests for the update notifier's GitHub releases lookup. Sibling
//! file loaded via `#[path]`; the `_test.rs` suffix matches
//! `tools/crap-score.mjs`'s filter, so these tests stay out of the
//! production file's CRAP roll-up. Moving them back inline puts that
//! file's score at the mercy of its test count.

use super::*;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn valid_release_json() -> serde_json::Value {
    serde_json::json!({
        "tag_name": "v0.5.0",
        "name": "v0.5.0 — newer",
        "html_url": "https://github.com/JoaoPucci/ani-gui/releases/tag/v0.5.0",
        "published_at": "2026-06-01T00:00:00Z",
        "body": "release notes",
        "draft": false,
        "prerelease": false
    })
}

#[test]
fn parse_array_when_including_prereleases() {
    let body = serde_json::to_vec(&serde_json::json!([valid_release_json()])).unwrap();
    let release = parse_response(&body, true).expect("release");
    assert_eq!(release.tag, "v0.5.0");
    assert_eq!(release.name, "v0.5.0 — newer");
    assert_eq!(release.body, "release notes");
}

#[test]
fn parse_single_object_when_excluding_prereleases() {
    let body = serde_json::to_vec(&valid_release_json()).unwrap();
    let release = parse_response(&body, false).expect("release");
    assert_eq!(release.tag, "v0.5.0");
}

#[test]
fn drafts_are_filtered_to_none() {
    let mut payload = valid_release_json();
    payload["draft"] = serde_json::Value::Bool(true);
    let body = serde_json::to_vec(&payload).unwrap();
    assert!(parse_response(&body, false).is_none());
}

#[test]
fn missing_tag_or_url_returns_none() {
    let mut no_tag = valid_release_json();
    no_tag.as_object_mut().unwrap().remove("tag_name");
    let no_tag_bytes = serde_json::to_vec(&no_tag).unwrap();
    assert!(parse_response(&no_tag_bytes, false).is_none());

    let mut no_url = valid_release_json();
    no_url.as_object_mut().unwrap().remove("html_url");
    let no_url_bytes = serde_json::to_vec(&no_url).unwrap();
    assert!(parse_response(&no_url_bytes, false).is_none());
}

#[test]
fn empty_array_returns_none() {
    let body = serde_json::to_vec(&serde_json::Value::Array(vec![])).unwrap();
    assert!(parse_response(&body, true).is_none());
}

#[test]
fn malformed_json_returns_none() {
    assert!(parse_response(b"not-json", false).is_none());
    assert!(parse_response(b"not-json", true).is_none());
}

#[test]
fn name_falls_back_to_tag_when_missing() {
    let mut payload = valid_release_json();
    payload.as_object_mut().unwrap().remove("name");
    let body = serde_json::to_vec(&payload).unwrap();
    let release = parse_response(&body, false).expect("release");
    assert_eq!(release.name, "v0.5.0");
}

#[tokio::test]
async fn fetch_hits_list_endpoint_when_including_prereleases() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO_PATH}/releases")))
        .and(query_param("per_page", "1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([valid_release_json()])),
        )
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let release = fetch_latest_release(&client, true, Some(&server.uri())).await;
    assert!(release.is_some());
    assert_eq!(release.unwrap().tag, "v0.5.0");
}

#[tokio::test]
async fn fetch_hits_latest_endpoint_when_excluding_prereleases() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO_PATH}/releases/latest")))
        .respond_with(ResponseTemplate::new(200).set_body_json(valid_release_json()))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let release = fetch_latest_release(&client, false, Some(&server.uri())).await;
    assert!(release.is_some());
}

#[tokio::test]
async fn fetch_returns_none_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    assert!(fetch_latest_release(&client, true, Some(&server.uri()))
        .await
        .is_none());
    assert!(fetch_latest_release(&client, false, Some(&server.uri()))
        .await
        .is_none());
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
