//! Kitsu ids the renderer supplies. A play, a watch, a skip-time
//! lookup, a title match and a detail read each take a Kitsu id from
//! the page they were made from, and each writes something keyed by
//! it: the history row and its page, the show's mapping and its played
//! mark, the skip times, the title match, the gone mark. Only digits
//! are a Kitsu id; anything else is no id, and writes nothing under
//! one.

use super::*;
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wiremock::MockServer;

const NOT_IDS: [&str; 4] = ["../49877", "49877/x", "12:21", "kid-1"];

fn state_at(dir: &Path, kitsu_uri: &str) -> AppState {
    AppState {
        anidb_base: None,
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: dir.join("history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: dir.join("state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// A watch of a show whose title carries no cour, so the cour guard
/// accepts any id without asking Kitsu.
fn seitokai() -> Watch {
    Watch {
        show_id: "hianime:seitokai-10497".into(),
        title: "Seitokai ni mo Ana wa Aru!".into(),
        ep_no: "1".into(),
    }
}

async fn requests(kitsu: &MockServer) -> usize {
    kitsu.received_requests().await.unwrap_or_default().len()
}

/// The finding's case: an id that is not digits used to pass to the
/// mapping write, which stored it as a mapping a play stored — one no
/// guess may replace — on a row that records no id.
#[tokio::test]
async fn a_watch_with_an_id_that_is_not_digits_writes_no_mapping() {
    for not_id in NOT_IDS {
        let kitsu = MockServer::start().await;
        let td = tempfile::tempdir().expect("tempdir");
        let state = state_at(td.path(), &kitsu.uri());
        let watch = seitokai();

        record_watch(&state, &watch, Some(not_id)).await;

        let rows = crate::history::read_all(&state.history_path).expect("rows");
        assert_eq!(rows.len(), 1, "{not_id}: the watch is still recorded");
        assert_eq!(rows[0].kitsu_id, None, "{not_id}");
        assert_eq!(
            crate::commands::kitsu::allmanga_kitsu_get(&state, &watch.show_id).expect("get"),
            None,
            "{not_id}: no mapping"
        );
        assert!(
            !crate::commands::kitsu_played::mapping_played(&state, &watch.show_id).expect("played"),
            "{not_id}: no played mark"
        );
        assert_eq!(requests(&kitsu).await, 0, "{not_id}");
    }
}

/// An id with stray whitespace is the id it carries: the row, the
/// mapping and its played mark all name the digits.
#[tokio::test]
async fn a_watch_with_a_padded_id_records_the_digits() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), &kitsu.uri());
    let watch = seitokai();

    record_watch(&state, &watch, Some(" 49877 ")).await;

    let rows = crate::history::read_all(&state.history_path).expect("rows");
    assert_eq!(rows[0].kitsu_id.as_deref(), Some("49877"));
    assert_eq!(
        crate::commands::kitsu::allmanga_kitsu_get(&state, &watch.show_id)
            .expect("get")
            .as_deref(),
        Some("49877")
    );
    assert_eq!(
        crate::commands::kitsu_played::played_mapping(&state, &watch.show_id)
            .expect("played")
            .as_deref(),
        Some("49877")
    );
}

/// A play names its page by the id it was asked from; a page that is
/// not an id is no page.
#[test]
fn a_play_asked_from_a_page_that_is_not_an_id_names_no_page() {
    let td = tempfile::tempdir().expect("tempdir");
    let history = td.path().join("history");
    for not_id in NOT_IDS {
        let asked = crate::history::guard::Asked::now(&history, Some(not_id));
        assert_eq!(asked.page, None, "{not_id}");
    }
    let asked = crate::history::guard::Asked::now(&history, Some("49877"));
    assert_eq!(asked.page, Some("49877"));
}

/// Skip times are keyed by the Kitsu id they were asked for; an id
/// that is not digits asks nothing and stores nothing.
#[tokio::test]
async fn skip_times_for_an_id_that_is_not_digits_ask_and_store_nothing() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), &kitsu.uri());

    for not_id in NOT_IDS {
        let got = crate::commands::aniskip::aniskip_get(&state, not_id, "1", 1440.0)
            .await
            .expect("no skip times, not an error");
        assert!(got.is_empty(), "{not_id}");
    }

    assert_eq!(requests(&kitsu).await, 0);
}

/// A title match stores the Kitsu id a search settled on; one that is
/// not digits is not stored.
#[test]
fn a_title_match_naming_an_id_that_is_not_digits_is_not_stored() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), "http://127.0.0.1:9");
    let watch = seitokai();
    crate::history::write_atomic(
        &state.history_path,
        &[crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: watch.show_id.clone(),
            title: watch.title.clone(),
            watched_at: None,
            kitsu_id: None,
        }],
    )
    .unwrap();
    let provider = crate::scraper::provider::ShowKey::parse(&watch.show_id).provider;

    for not_id in NOT_IDS {
        crate::commands::title_match_store::store_title_match(
            &state,
            provider,
            &watch.title,
            1,
            not_id,
        )
        .expect("ignored, not an error");
        assert_eq!(
            crate::commands::kitsu::title_match_get(&state, provider, &watch.title, 1)
                .expect("get"),
            None,
            "{not_id}"
        );
    }
}

/// A gone mark is kept under the Kitsu id a detail read was answered
/// for; a read of something that is not an id marks nothing.
#[test]
fn a_404_for_an_id_that_is_not_digits_marks_nothing() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), "http://127.0.0.1:9");
    let gone = crate::error::AniError::Upstream { status: 404 };

    for not_id in NOT_IDS {
        let begun = crate::history::guard::epoch(&state.history_path);
        crate::commands::kitsu_gone::note_failure(&state, begun, not_id, &gone);
        assert!(
            !crate::commands::kitsu_gone::is_gone(&state, not_id).expect("read"),
            "{not_id}"
        );
    }
    assert!(
        !crate::commands::kitsu_gone::is_gone(&state, "49877").expect("read"),
        "nothing marked under digits the value carried"
    );
}

/// A 404 speaks only for the value Kitsu was asked for. The detail
/// read trims before asking and passes the digits; a padded value
/// reaching the mark was not a read of those digits, so the id a
/// history row recorded is not marked gone, and its detail row stays.
#[test]
fn a_404_for_a_padded_id_marks_nothing_under_its_digits() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), "http://127.0.0.1:9");
    let key = crate::commands::kitsu::anime_detail_key("49877");
    crate::cache::meta_cache_put(&state.cache_pool, &key, "{}", 3600).unwrap();
    let gone = crate::error::AniError::Upstream { status: 404 };

    let begun = crate::history::guard::epoch(&state.history_path);
    crate::commands::kitsu_gone::note_failure(&state, begun, " 49877 ", &gone);

    assert!(!crate::commands::kitsu_gone::is_gone(&state, "49877").expect("read"));
    assert!(crate::cache::meta_cache_get(&state.cache_pool, &key)
        .expect("read")
        .is_some());
}
