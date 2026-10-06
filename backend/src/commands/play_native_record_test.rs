//! What a recorded watch writes, and in what order.

use super::*;
use crate::meta::kitsu::KitsuAnimeRef;
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

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

/// Seed the Kitsu detail cache so the cour guard reads `slug` for `id`
/// without a request.
fn cache_detail(state: &AppState, id: &str, slug: &str) {
    let detail = KitsuAnimeRef {
        id: id.into(),
        canonical_title: "Stone Ocean".into(),
        titles: std::collections::HashMap::new(),
        abbreviated_titles: Vec::new(),
        slug: Some(slug.into()),
        synopsis: None,
        start_date: None,
        end_date: None,
        episode_count: Some(12),
        average_rating: None,
        subtype: Some("ONA".into()),
        status: Some("finished".into()),
        age_rating: None,
        popularity_rank: None,
        poster_image: None,
        cover_image: None,
    };
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key(id),
        &serde_json::to_string(&detail).expect("ser"),
        3600,
    )
    .expect("seed detail");
}

fn part_two() -> Watch {
    Watch {
        show_id: "hianime:stone-ocean-part-2-100".into(),
        title: "JoJo no Kimyou na Bouken Part 6: Stone Ocean Part 2".into(),
        ep_no: "1".into(),
    }
}

fn row_id(state: &AppState, show_id: &str) -> Option<String> {
    crate::history::read_all(&state.history_path)
        .expect("rows")
        .into_iter()
        .find(|r| r.id == show_id)
        .and_then(|r| r.kitsu_id)
}

/// The row and its stamp are written before the cour guard's Kitsu
/// read, so a slow or failing Kitsu neither delays the watch nor, when
/// the app closes meanwhile, loses it. The guard is read once.
#[tokio::test]
async fn a_slow_kitsu_does_not_hold_the_row_back() {
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&kitsu)
        .await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(state_at(td.path(), &kitsu.uri()));
    // A title with a cour is one the guard reads Kitsu for.
    let watch = part_two();

    let recording = {
        let state = Arc::clone(&state);
        let watch = watch.clone();
        tokio::spawn(async move { record_watch(&state, &watch, Some("45412")).await })
    };
    tokio::time::sleep(Duration::from_millis(500)).await;

    let rows = crate::history::read_all(&state.history_path).expect("rows");
    let row = rows.iter().find(|r| r.id == watch.show_id);
    assert!(row.is_some(), "the row is on disk while the guard waits");
    assert_eq!(
        row.and_then(|r| r.kitsu_id.clone()),
        None,
        "an id the guard has not judged is not on the row"
    );
    assert!(
        crate::commands::kitsu::watched_at_get(&state, &watch.show_id)
            .expect("stamp read")
            .is_some(),
        "the stamp is written while the guard waits"
    );
    recording.await.expect("recording");
    // A failed read is no evidence against the pairing: the id stands.
    assert_eq!(row_id(&state, &watch.show_id).as_deref(), Some("45412"));
}

/// The Kitsu detail the guard reads, served after `delay`, naming
/// `slug` as the entry's.
async fn serve_detail(kitsu: &MockServer, id: &str, slug: &str, delay: Duration) {
    let mut body: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/kitsu/anime_one_piece_detail.json"
    ))
    .expect("fixture");
    body["data"]["id"] = serde_json::Value::from(id);
    body["data"]["attributes"]["slug"] = serde_json::Value::from(slug);
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(body)
                .set_delay(delay),
        )
        .mount(kitsu)
        .await;
}

/// An id the guard refuses is never on the row, not even while the
/// guard waits on Kitsu: a Continue load in that window would render
/// the refused cour.
#[tokio::test]
async fn a_refused_id_is_never_on_the_row() {
    let kitsu = MockServer::start().await;
    serve_detail(
        &kitsu,
        "44294",
        "jojo-no-kimyou-na-bouken-stone-ocean",
        Duration::from_secs(1),
    )
    .await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(state_at(td.path(), &kitsu.uri()));
    let watch = part_two();

    let recording = {
        let state = Arc::clone(&state);
        let watch = watch.clone();
        tokio::spawn(async move { record_watch(&state, &watch, Some("44294")).await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        row_id(&state, &watch.show_id),
        None,
        "while the guard waits"
    );
    recording.await.expect("recording");
    assert_eq!(row_id(&state, &watch.show_id), None, "once it refused");
}

/// A pairing the cache can already judge goes on the row in the same
/// write as the watch.
#[tokio::test]
async fn a_pairing_the_cache_accepts_goes_on_with_the_row() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), &kitsu.uri());
    cache_detail(
        &state,
        "45412",
        "jojo-no-kimyou-na-bouken-stone-ocean-part-2",
    );
    let watch = part_two();

    record_watch(&state, &watch, Some("45412")).await;

    assert_eq!(row_id(&state, &watch.show_id).as_deref(), Some("45412"));
    assert!(kitsu
        .received_requests()
        .await
        .unwrap_or_default()
        .is_empty());
}

/// A play the cour guard refuses records no id, and a stale id the
/// row held that disagrees with the title the same way goes too —
/// the row's counterpart of the mapping the refusal drops.
#[tokio::test]
async fn a_refused_play_clears_a_stale_id_the_title_disagrees_with() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), &kitsu.uri());
    cache_detail(&state, "44294", "jojo-no-kimyou-na-bouken-stone-ocean");
    let watch = part_two();
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: watch.show_id.clone(),
            title: watch.title.clone(),
            watched_at: None,
            kitsu_id: Some("44294".into()),
        },
    )
    .expect("seed row");

    record_watch(&state, &watch, Some("44294")).await;

    assert_eq!(row_id(&state, &watch.show_id), None);
}

/// A refused play leaves a recorded id the title agrees with.
#[tokio::test]
async fn a_refused_play_keeps_an_id_the_title_agrees_with() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), &kitsu.uri());
    cache_detail(&state, "44294", "jojo-no-kimyou-na-bouken-stone-ocean");
    cache_detail(
        &state,
        "45412",
        "jojo-no-kimyou-na-bouken-stone-ocean-part-2",
    );
    let watch = part_two();
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: watch.show_id.clone(),
            title: watch.title.clone(),
            watched_at: None,
            kitsu_id: Some("45412".into()),
        },
    )
    .expect("seed row");

    record_watch(&state, &watch, Some("44294")).await;

    assert_eq!(row_id(&state, &watch.show_id).as_deref(), Some("45412"));
}
