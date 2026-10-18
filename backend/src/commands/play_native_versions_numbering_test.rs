//! A season listed twice — an "(Uncensored)" copy and a plain one —
//! whose listing a later numbering re-tags: each copy is a show key of
//! its own, with a display stamp and former tags of its own, and every
//! reader of either copy's numbering reads it as a single copy's would
//! be read.

use crate::app::AppState;
use crate::commands::play::PlayArgs;
use crate::commands::play_native_versions::VersionKind;
use crate::commands::play_resolution_cache;
use crate::scraper::provider::{BrowseHit, EpisodeRef, ShowKey};
use crate::scraper::table_provider::TableProvider;

pub(crate) const TITLE: &str = "Cumulative Show";
pub(crate) const PLAIN: &str = "cumulative-show-66";
pub(crate) const UNCUT: &str = "cumulative-show-68";
const MASTER: &str = "https://cdn.example/x/master.m3u8";

fn row(id: u64, number: u32, tag: &str) -> EpisodeRef {
    EpisodeRef {
        id,
        number,
        number2: Some(tag.into()),
        former_tag: None,
        title: None,
    }
}

fn hit(slug: &str, title: &str) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
        jname: None,
    }
}

/// A continuation the provider numbers on from 13 that opens on a recap
/// tagged "12.5" — the entry's 0.5, which an earlier version called
/// "12.5" — listed twice, every row of both copies streaming in sub.
pub(crate) fn two_cumulative_copies() -> TableProvider {
    let listing = |base: u64| {
        vec![
            row(base + 1, 1, "12.5"),
            row(base + 2, 2, "13"),
            row(base + 3, 3, "14"),
        ]
    };
    let mut table = TableProvider::new()
        .hits(
            TITLE,
            vec![
                hit(PLAIN, TITLE),
                hit(UNCUT, &format!("{TITLE} (Uncensored)")),
            ],
        )
        .listing(66, listing(6600))
        .listing(68, listing(6800))
        .year(66, Some(2026))
        .year(68, Some(2026));
    for id in [6601, 6602, 6603, 6801, 6802, 6803] {
        table = table.stream(id, "sub", MASTER).mode(id, "sub", true);
    }
    table
}

fn key(slug: &str) -> String {
    ShowKey::new(crate::provider_set::ProviderSet::TEST_DEFAULT, slug).to_string()
}

fn state(td: &tempfile::TempDir) -> AppState {
    let mut state = crate::commands::play::tests::state_with_proxy_origin();
    state
        .providers
        .set_stub(std::sync::Arc::new(two_cumulative_copies()));
    state.config_path = td.path().join("config.toml");
    state.history_path = td.path().join("history");
    state
}

fn ask(episode: &str, version: Option<&str>) -> PlayArgs {
    let mut json = serde_json::json!({
        "title": TITLE,
        "episode": episode,
        "mode": "sub",
        "episode_count": 2,
        "year": 2026,
        "kitsu_id": "88",
    });
    if let Some(v) = version {
        json["version"] = v.into();
    }
    serde_json::from_value(json).expect("args")
}

#[tokio::test]
async fn either_copy_asked_for_by_a_former_number_plays_as_itself() {
    for version in [None, Some("standard"), Some("uncensored")] {
        let td = tempfile::tempdir().expect("td");
        let state = state(&td);
        let resp = crate::commands::play::play_with_progress(&state, &ask("12.5", version), |_| {})
            .await
            .expect("plays the recap");
        assert_eq!(resp.episode.as_deref(), Some("0.5"), "{version:?}");
        assert_eq!(resp.former_episode.as_deref(), Some("12.5"), "{version:?}");
    }
}

#[tokio::test]
async fn a_play_by_a_former_number_is_kept_under_the_version_it_named() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    let asked = ask("12.5", Some("standard"));
    crate::commands::play::play_with_progress(&state, &asked, |_| {})
        .await
        .expect("plays the recap");
    let under = |episode: &str, version: Option<VersionKind>| {
        play_resolution_cache::get(
            &state.cache_pool,
            &play_resolution_cache::cache_key_for_version(
                TITLE,
                "sub",
                "best",
                episode,
                asked.year,
                asked.episode_count,
                None,
                version,
            ),
        )
        .expect("readable")
    };
    let kept = under("0.5", Some(VersionKind::Standard)).expect("kept under the version asked");
    assert_eq!(kept.show_id, key(PLAIN));
    assert!(
        under("0.5", None).is_none(),
        "the plain copy's row answers no request naming no version"
    );
}

#[tokio::test]
async fn a_probe_puts_every_copys_stamp_right() {
    let td = tempfile::tempdir().expect("td");
    let state = state(&td);
    // Each copy's history row was stamped by an earlier version, which
    // tagged the recap "24.5".
    for slug in [PLAIN, UNCUT] {
        crate::commands::numbering_offset::put_display(&state, &key(slug), 12, 1, "24.5");
    }
    let args = crate::commands::availability::AvailabilityArgs {
        title: TITLE.into(),
        mode: "sub".into(),
        alt_titles: Vec::new(),
        episode_count: Some(2),
        year: Some(2026),
        subtype: None,
        kitsu_id: Some("88".into()),
        status: Some("finished".into()),
        background: false,
        bypass_cache: false,
    };
    let probed = crate::commands::availability::check_availability(&state, &args)
        .await
        .expect("answered");
    assert_eq!(probed.versions.len(), 2, "the probe sees both copies");
    for slug in [UNCUT, PLAIN] {
        assert_eq!(
            crate::commands::numbering_offset::display_stamp(&state, &key(slug)),
            Some((1, "12.5".to_string())),
            "{slug}: the stamp names the recap by its tag now"
        );
    }
}
