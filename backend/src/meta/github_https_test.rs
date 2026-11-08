//! The update answer's release page must be an https URL
//! (`normalise`). Sibling file loaded via `#[path]`; the `_test.rs`
//! suffix keeps these cases out of `github.rs`'s CRAP roll-up.

use super::*;

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

fn release_with_page(url: &str) -> Vec<u8> {
    let mut release = valid_release_json();
    release["html_url"] = serde_json::Value::String(url.to_owned());
    serde_json::to_vec(&release).unwrap()
}

#[test]
fn a_release_whose_page_is_not_an_https_page_is_not_offered() {
    // The dialog links to `url`. Anything but an https page there
    // means the payload is not a release page, and the notice is
    // not shown.
    for url in [
        "http://github.com/JoaoPucci/ani-gui/releases/tag/v0.5.0",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "app://localhost/settings",
        "not a url",
        "",
    ] {
        assert!(
            parse_response(&release_with_page(url), false).is_none(),
            "{url}"
        );
    }
    let https = "https://github.com/JoaoPucci/ani-gui/releases/tag/v0.5.0";
    assert_eq!(
        parse_response(&release_with_page(https), false).map(|r| r.url),
        Some(https.to_owned())
    );
}

proptest::proptest! {
    /// A release is offered exactly when its page is an https URL.
    #[test]
    fn only_an_https_page_is_offered(
        scheme in proptest::sample::select(vec!["https", "http", "ftp", "file", "javascript", "app", "data"]),
        host in "[a-z]{1,8}",
        rest in "[a-z0-9/._-]{0,16}",
    ) {
        let url = format!("{scheme}://{host}/{rest}");
        let offered = parse_response(&release_with_page(&url), false).is_some();
        proptest::prop_assert_eq!(offered, scheme == "https");
    }
}
