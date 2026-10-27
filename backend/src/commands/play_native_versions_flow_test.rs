//! Versions of one pick through the answers that carry it: the
//! resolve, the request shapes that name a version, the resolution
//! cache's key and row, and the session response.

use super::*;
use crate::commands::play_native_resolve::{resolve_native, NativeResolveRequest};
use crate::scraper::provider::ShowKey;
use crate::scraper::table_provider::{listing_of, TableProvider};

const YEAR: u32 = 2020;
const PLAIN: &str = "https://cdn.example/plain/master.m3u8";
const TAGGED: &str = "https://cdn.example/uncensored/master.m3u8";

fn key(slug: &str) -> String {
    ShowKey::new(crate::provider_set::ProviderSet::TEST_DEFAULT, slug).to_string()
}

fn card(slug: &str, title: &str) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
        jname: None,
    }
}

/// "The Show" listed twice, three episodes each, both from the
/// entry's year; episode 2 of each streams from its own master.
fn two_copies() -> TableProvider {
    TableProvider::new()
        .search_default(vec![
            card("the-show-1", "The Show"),
            card("the-show-2", "The Show (Uncensored)"),
        ])
        .listing(1, listing_of(1, 3))
        .listing(2, listing_of(2, 3))
        .year(1, Some(YEAR))
        .year(2, Some(YEAR))
        .stream(1002, "sub", PLAIN)
        .mode(1002, "sub", true)
        .stream(2002, "sub", TAGGED)
        .mode(2002, "sub", true)
}

async fn resolve(
    provider: &TableProvider,
    version: Option<VersionKind>,
) -> crate::commands::play_native_resolve::NativeResolved {
    resolve_native(
        provider,
        NativeResolveRequest {
            title: "The Show",
            alt_titles: &[],
            episode: "2",
            mode: "sub",
            quality: "best",
            expected_count: Some(3),
            year: Some(YEAR),
            subtype: Some("TV"),
            version,
            stamps_beside: None,
            tail: None,
            heard: None,
        },
        &mut |_| {},
    )
    .await
    .expect("resolved")
}

fn both_versions() -> Vec<VersionInfo> {
    vec![
        VersionInfo {
            kind: VersionKind::Standard,
            show_key: key("the-show-1"),
            has_mode: None,
        },
        VersionInfo {
            kind: VersionKind::Uncensored,
            show_key: key("the-show-2"),
            has_mode: None,
        },
    ]
}

#[tokio::test]
async fn a_resolve_naming_no_version_plays_the_uncensored_one_and_names_both() {
    let got = resolve(&two_copies(), None).await;
    assert_eq!(got.slug, key("the-show-2"));
    assert_eq!(got.master_url, TAGGED);
    assert_eq!(got.versions, both_versions());
    assert_eq!(got.version, Some(VersionKind::Uncensored));
}

#[tokio::test]
async fn a_resolve_naming_the_standard_version_plays_the_plain_copy() {
    let got = resolve(&two_copies(), Some(VersionKind::Standard)).await;
    assert_eq!(got.slug, key("the-show-1"));
    assert_eq!(got.master_url, PLAIN);
    assert_eq!(got.versions, both_versions());
    assert_eq!(got.version, Some(VersionKind::Standard));
}

#[tokio::test]
async fn a_show_with_one_version_names_none_whatever_is_asked() {
    let provider = TableProvider::new()
        .search_default(vec![card("the-show-1", "The Show")])
        .listing(1, listing_of(1, 3))
        .year(1, Some(YEAR))
        .stream(1002, "sub", PLAIN)
        .mode(1002, "sub", true);
    let got = resolve(&provider, Some(VersionKind::Uncensored)).await;
    assert_eq!(got.slug, key("the-show-1"));
    assert!(got.versions.is_empty());
    assert_eq!(got.version, None);
}

#[test]
fn a_play_and_a_download_ask_for_the_version_they_name() {
    let play: crate::commands::play::PlayArgs = serde_json::from_value(serde_json::json!({
        "title": "The Show", "episode": "2", "mode": "sub", "version": "standard",
    }))
    .expect("play args");
    assert_eq!(play.version, Some(VersionKind::Standard));
    assert_eq!(
        play.native_request("best", None, None).version,
        Some(VersionKind::Standard)
    );
    let download: crate::commands::download::DownloadArgs =
        serde_json::from_value(serde_json::json!({
            "title": "The Show", "episode": "2", "mode": "sub", "version": "uncensored",
        }))
        .expect("download args");
    assert_eq!(
        download.native_request("best", None, None).version,
        Some(VersionKind::Uncensored)
    );
}

#[test]
fn the_stream_endpoints_read_a_version_from_the_query() {
    let play: crate::commands::play::PlayArgs =
        serde_urlencoded::from_str("title=The+Show&episode=2&mode=sub&version=uncensored")
            .expect("query");
    assert_eq!(play.version, Some(VersionKind::Uncensored));
    let download: crate::commands::download::DownloadArgs =
        serde_urlencoded::from_str("title=The+Show&episode=2&mode=sub&version=standard")
            .expect("query");
    assert_eq!(download.version, Some(VersionKind::Standard));
}

#[test]
fn the_resolution_cache_keys_a_version_apart() {
    use crate::commands::play_resolution_cache::{cache_key, cache_key_for_version};
    let at = |v| {
        cache_key_for_version(
            "The Show",
            "sub",
            "best",
            "2",
            Some(YEAR),
            Some(3),
            Some("TV"),
            v,
        )
    };
    // A request naming no version keeps the key it had.
    assert_eq!(
        at(None),
        cache_key(
            "The Show",
            "sub",
            "best",
            "2",
            Some(YEAR),
            Some(3),
            Some("TV")
        )
    );
    let standard = at(Some(VersionKind::Standard));
    let uncensored = at(Some(VersionKind::Uncensored));
    assert_ne!(standard, uncensored);
    assert_ne!(standard, at(None));
    assert_ne!(uncensored, at(None));
}

/// A row resolved before picks had versions holds whichever copy the
/// pool's tie happened to pick, under a key a request naming no
/// version still reads. Master keys its rows `play:v21:` for the
/// appended-extras reading and `play:v22:` for the recaps a
/// continuation's offset renumbered, neither of whose pickers names
/// versions, so this key is past both.
#[test]
fn a_row_resolved_before_versions_is_not_read() {
    let key = crate::commands::play_resolution_cache::cache_key(
        "t", "sub", "best", "1", None, None, None,
    );
    for before in ["play:v20:", "play:v21:", "play:v22:"] {
        assert!(
            !key.starts_with(before),
            "the key must have moved past {before}: {key}"
        );
    }
}

#[tokio::test]
async fn the_cache_row_and_the_session_answer_carry_the_versions() {
    let native = resolve(&two_copies(), Some(VersionKind::Standard)).await;
    let row = crate::commands::play::cached_resolution_for(&native);
    assert_eq!(row.versions, both_versions());
    assert_eq!(row.version, Some(VersionKind::Standard));
    let body = serde_json::to_value(&row).expect("row");
    let back: crate::commands::play_resolution_cache::CachedResolution =
        serde_json::from_value(body).expect("row back");
    assert_eq!(back, row);
}

#[test]
fn a_session_answer_names_versions_only_when_there_are_several() {
    let mut resp = crate::commands::session::CreateSessionResponse {
        session_id: "s".into(),
        media_url: "http://127.0.0.1/s/x/master.m3u8".into(),
        media_kind: crate::proxy::MediaKind::Hls,
        cache_hit: false,
        subtitles: Vec::new(),
        versions: Vec::new(),
        version: None,
        episode: None,
        former_episode: None,
        episode_unverified: false,
    };
    let plain = serde_json::to_value(&resp).expect("json");
    assert!(plain.get("versions").is_none() && plain.get("version").is_none());
    resp.versions = both_versions();
    resp.version = Some(VersionKind::Uncensored);
    let json = serde_json::to_value(&resp).expect("json");
    assert_eq!(
        json["versions"],
        serde_json::json!([
            {"kind": "standard", "show_key": key("the-show-1")},
            {"kind": "uncensored", "show_key": key("the-show-2")},
        ])
    );
    assert_eq!(json["version"], "uncensored");
}

#[test]
fn a_session_built_from_a_resolve_carries_its_versions() {
    let native = crate::commands::play_native_resolve::NativeResolved {
        slug: key("the-show-2"),
        provider: crate::provider_set::ProviderSet::TEST_DEFAULT,
        title: "The Show (Uncensored)".into(),
        master_url: TAGGED.into(),
        referer: None,
        subtitles: Vec::new(),
        episode_cap: Some(3),
        numbering_offset: 0,
        extra_tags: Vec::new(),
        resolved_slot: 2,
        resolved_tag: None,
        versions: both_versions(),
        version: Some(VersionKind::Uncensored),
        former_tags: Vec::new(),
        other_copies: Vec::new(),
        unread_past: None,
    };
    let mut resp = crate::commands::session::CreateSessionResponse {
        session_id: "s".into(),
        media_url: "http://127.0.0.1/s/x/master.m3u8".into(),
        media_kind: crate::proxy::MediaKind::Hls,
        cache_hit: false,
        subtitles: Vec::new(),
        versions: Vec::new(),
        version: None,
        episode: None,
        former_episode: None,
        episode_unverified: false,
    };
    crate::commands::play::with_versions_of(&mut resp, &native.versions, native.version);
    assert_eq!(resp.versions, both_versions());
    assert_eq!(resp.version, Some(VersionKind::Uncensored));
}

/// "The Show" listed twice, three episodes each, dubbed only in the
/// plain copy: every row carries sub, only the plain copy's carry dub,
/// and episode 2 streams in each mode its copy carries.
fn dubbed_plain_copy() -> TableProvider {
    let table = two_copies().stream(1002, "dub", PLAIN).stream_from(
        2002,
        "dub",
        crate::scraper::table_provider::Answer::fails(|| crate::error::AniError::NoResults),
    );
    [1001, 1002, 1003, 2001, 2002, 2003]
        .into_iter()
        .fold(table, |t, row| {
            t.mode(row, "sub", true).mode(row, "dub", row < 2000)
        })
}

async fn resolve_in(
    provider: &TableProvider,
    mode: &str,
    version: Option<VersionKind>,
) -> crate::commands::play_native_resolve::NativeResolved {
    resolve_native(
        provider,
        NativeResolveRequest {
            title: "The Show",
            alt_titles: &[],
            episode: "2",
            mode,
            quality: "best",
            expected_count: Some(3),
            year: Some(YEAR),
            subtype: Some("TV"),
            version,
            stamps_beside: None,
            tail: None,
            heard: None,
        },
        &mut |_| {},
    )
    .await
    .expect("resolved")
}

#[tokio::test]
async fn a_mode_only_the_plain_copy_carries_plays_from_it() {
    // The uncensored copy is the default, and it has no dub: the
    // resolve plays the dub from the copy that carries one rather than
    // answering that the episode has none.
    let provider = dubbed_plain_copy();
    for asked in [
        None,
        Some(VersionKind::Uncensored),
        Some(VersionKind::Standard),
    ] {
        let got = resolve_in(&provider, "dub", asked).await;
        assert_eq!(got.slug, key("the-show-1"), "{asked:?}");
        assert_eq!(got.master_url, PLAIN, "{asked:?}");
        assert_eq!(got.version, Some(VersionKind::Standard), "{asked:?}");
    }
    // In a mode every copy carries, the version asked for plays.
    let got = resolve_in(&provider, "sub", None).await;
    assert_eq!(got.slug, key("the-show-2"));
}
