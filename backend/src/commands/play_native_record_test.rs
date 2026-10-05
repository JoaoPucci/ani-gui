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

// — a removal while a watch is still being recorded ————————————————
//
// A recorded watch writes its row and stamp at once and the rest after
// the cour guard has read Kitsu: the mapping, and the id on the row.
// The frontend does not wait for it, so the user can be back on the
// home page deleting the row while that read is still out. A removal
// wins over what a watch begun before it was still going to write.

/// Everything on disk that names `show_id`: its history row, its
/// numbering offset, and any cache entry whose key or value carries
/// the id.
fn left_behind(state: &AppState, show_id: &str) -> Vec<String> {
    let mut left = Vec::new();
    let rows = crate::history::read_all(&state.history_path).expect("rows");
    if rows.iter().any(|r| r.id == show_id) {
        left.push("history row".to_owned());
    }
    let offsets = state.history_path.with_file_name("ani-gui-offsets");
    if std::fs::read_to_string(offsets)
        .unwrap_or_default()
        .contains(show_id)
    {
        left.push("numbering offset".to_owned());
    }
    for (key, body) in
        crate::cache::meta_cache_entries_prefix(&state.cache_pool, "").expect("cache")
    {
        if key.contains(show_id) || body.contains(show_id) {
            left.push(key);
        }
    }
    left
}

/// A watch of Part 2 from its own page, its recording held on the
/// guard's Kitsu read for a second.
async fn recording_held_on_kitsu(
    state: &Arc<AppState>,
    kitsu: &MockServer,
) -> tokio::task::JoinHandle<()> {
    serve_detail(
        kitsu,
        "45412",
        "jojo-no-kimyou-na-bouken-stone-ocean-part-2",
        Duration::from_secs(1),
    )
    .await;
    let recording = {
        let state = Arc::clone(state);
        tokio::spawn(async move { record_watch(&state, &part_two(), Some("45412")).await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    recording
}

#[tokio::test]
async fn a_delete_while_the_guard_reads_kitsu_leaves_nothing_of_the_show() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(state_at(td.path(), &kitsu.uri()));
    let recording = recording_held_on_kitsu(&state, &kitsu).await;

    assert!(
        crate::commands::history::history_delete(&state, &part_two().show_id).expect("delete"),
        "the row was there to delete"
    );
    recording.await.expect("recording");

    assert_eq!(
        left_behind(&state, &part_two().show_id),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn a_clear_while_the_guard_reads_kitsu_leaves_nothing_of_the_show() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(state_at(td.path(), &kitsu.uri()));
    let recording = recording_held_on_kitsu(&state, &kitsu).await;

    crate::commands::history::history_clear(&state).expect("clear");
    recording.await.expect("recording");

    assert_eq!(
        left_behind(&state, &part_two().show_id),
        Vec::<String>::new()
    );
}

/// Only the removed show's pending writes are dropped: removing another
/// show while the guard reads Kitsu leaves this watch recorded whole —
/// the row with its id, the stamp and the mapping.
#[tokio::test]
async fn removing_another_show_leaves_a_pending_recording_whole() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(state_at(td.path(), &kitsu.uri()));
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: "one-piece-69".into(),
            title: "One Piece".into(),
            watched_at: None,
            kitsu_id: None,
        },
    )
    .expect("seed another row");
    let recording = recording_held_on_kitsu(&state, &kitsu).await;

    assert!(crate::commands::history::history_delete(&state, "one-piece-69").expect("delete"));
    recording.await.expect("recording");

    let show = part_two().show_id;
    assert_eq!(row_id(&state, &show).as_deref(), Some("45412"));
    assert!(crate::commands::kitsu::watched_at_get(&state, &show)
        .expect("stamp")
        .is_some());
    assert_eq!(
        crate::commands::kitsu::allmanga_kitsu_get(&state, &show)
            .expect("mapping")
            .as_deref(),
        Some("45412")
    );
}

/// While the guard reads Kitsu the watch's row carries no Kitsu id,
/// but a removal in that window still knows the page the watch was
/// recorded from: the skip times cached under it go with the row.
#[tokio::test]
async fn a_delete_while_the_guard_reads_kitsu_takes_the_pages_skip_times() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(state_at(td.path(), &kitsu.uri()));
    crate::cache::meta_cache_put(&state.cache_pool, "aniskip:v2:45412:9:1", "[]", 3600)
        .expect("seed skip times");
    let recording = recording_held_on_kitsu(&state, &kitsu).await;

    assert!(crate::commands::history::history_delete(&state, &part_two().show_id).expect("delete"));
    recording.await.expect("recording");

    assert_eq!(
        crate::cache::meta_cache_get(&state.cache_pool, "aniskip:v2:45412:9:1").expect("cache"),
        None
    );
}

/// A handoff's resolve can land on another provider's key than the row
/// the user removes while it runs. The watch recorded after the spawn
/// names the page it was asked from, and a show known by that page
/// removed since the request began is not recorded under the new key.
#[tokio::test]
async fn a_handoff_whose_show_was_removed_under_another_key_records_nothing() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), &kitsu.uri());
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: "hianime:the-show-100".into(),
            title: "The Show".into(),
            watched_at: None,
            kitsu_id: Some("77".into()),
        },
    )
    .expect("seed row");
    let requested = crate::history::guard::epoch(&state.history_path);
    assert!(
        crate::commands::history::history_delete(&state, "hianime:the-show-100").expect("delete")
    );

    let watch = Watch {
        show_id: "the-show-77".into(),
        title: "The Show".into(),
        ep_no: "1".into(),
    };
    record_watch_requested_at(&state, &watch, Some("77"), requested).await;

    assert_eq!(left_behind(&state, "the-show-77"), Vec::<String>::new());
}

/// Two watches of the same show from different pages overlap: A waits
/// on the guard's Kitsu read while B, whose verdict the cache already
/// has, writes and settles the row. The row and the mapping are B's;
/// A's verdict, landing after, is for a row that is no longer its.
#[tokio::test]
async fn a_later_watch_keeps_its_row_from_an_earlier_watchs_verdict() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(state_at(td.path(), &kitsu.uri()));
    cache_detail(
        &state,
        "45413",
        "jojo-no-kimyou-na-bouken-stone-ocean-part-2",
    );
    let a = recording_held_on_kitsu(&state, &kitsu).await;

    record_watch(&state, &part_two(), Some("45413")).await;
    assert_eq!(
        row_id(&state, &part_two().show_id).as_deref(),
        Some("45413")
    );
    a.await.expect("recording");

    assert_eq!(
        row_id(&state, &part_two().show_id).as_deref(),
        Some("45413"),
        "the row keeps the later watch's id"
    );
    assert_eq!(
        crate::commands::kitsu::allmanga_kitsu_get(&state, &part_two().show_id)
            .expect("read")
            .as_deref(),
        Some("45413"),
        "the mapping keeps the later watch's id"
    );
}

/// The refusal side of the same overlap: A's pairing is refused after
/// B has settled the row with a mapping B's own guard accepted. A's
/// verdict is about a row that is no longer its, so it does not drop
/// B's mapping, whatever A's title says of it.
#[tokio::test]
async fn a_later_watchs_mapping_survives_an_earlier_watchs_refusal() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = Arc::new(state_at(td.path(), &kitsu.uri()));
    // A, from a page whose slug names no part: refused for Part 2.
    serve_detail(
        &kitsu,
        "45412",
        "jojo-no-kimyou-na-bouken-stone-ocean",
        Duration::from_secs(1),
    )
    .await;
    let a = {
        let state = Arc::clone(&state);
        tokio::spawn(async move { record_watch(&state, &part_two(), Some("45412")).await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    // B, under a title that names no part, from a page the cache knows.
    cache_detail(&state, "45413", "jojo-no-kimyou-na-bouken-stone-ocean");
    let b = Watch {
        title: "Stone Ocean".into(),
        ..part_two()
    };
    record_watch(&state, &b, Some("45413")).await;
    let mapping =
        || crate::commands::kitsu::allmanga_kitsu_get(&state, &part_two().show_id).expect("read");
    assert_eq!(mapping().as_deref(), Some("45413"));
    a.await.expect("recording");

    assert_eq!(
        mapping().as_deref(),
        Some("45413"),
        "the later watch's mapping stands"
    );
}

/// The history row for `show_id`, as the file holds it.
fn row_of(state: &AppState, show_id: &str) -> crate::history::HistoryEntry {
    crate::history::read_all(&state.history_path)
        .expect("read history")
        .into_iter()
        .find(|e| e.id == show_id)
        .expect("row")
}

/// Two watches of one show overlap: A's request begins, then stalls —
/// on a cached stream's check, on a handoff's resolve — while B records
/// a later episode. When A reaches its write, the row and its stamp
/// are B's: a request begun before a watch was recorded does not
/// replace it.
#[tokio::test]
async fn an_earlier_request_does_not_replace_a_later_watch() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), &kitsu.uri());
    let requested = crate::history::guard::epoch(&state.history_path);
    let b = Watch {
        ep_no: "5".into(),
        ..part_two()
    };
    record_watch(&state, &b, None).await;
    let b_stamp = row_of(&state, &b.show_id).watched_at;
    tokio::time::sleep(Duration::from_millis(5)).await;

    let a = Watch {
        ep_no: "3".into(),
        ..part_two()
    };
    record_watch_requested_at(&state, &a, None, requested).await;

    let row = row_of(&state, &b.show_id);
    assert_eq!(row.ep_no, "5", "the row keeps the later watch's episode");
    assert_eq!(
        row.watched_at, b_stamp,
        "the row keeps the later watch's moment"
    );
    assert_eq!(
        crate::commands::kitsu::watched_at_get(&state, &b.show_id).expect("read"),
        b_stamp,
        "the stamp keeps the later watch's moment"
    );
}

/// What a play's resolve writes between a watch's request and its
/// recording — the row's episode, with neither a watch moment nor a
/// Kitsu id — is not another watch, and the watch still records.
#[tokio::test]
async fn a_resolve_written_while_a_watch_waits_does_not_drop_it() {
    let kitsu = MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_at(td.path(), &kitsu.uri());
    let requested = crate::history::guard::epoch(&state.history_path);
    let watch = Watch {
        ep_no: "3".into(),
        ..part_two()
    };
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "3".into(),
            id: watch.show_id.clone(),
            title: watch.title.clone(),
            watched_at: None,
            kitsu_id: None,
        },
    )
    .expect("resolve row");

    record_watch_requested_at(&state, &watch, None, requested).await;

    let row = row_of(&state, &watch.show_id);
    assert_eq!(row.ep_no, "3");
    assert!(row.watched_at.is_some(), "the watch is recorded");
    assert_eq!(
        crate::commands::kitsu::watched_at_get(&state, &watch.show_id).expect("read"),
        row.watched_at
    );
}
