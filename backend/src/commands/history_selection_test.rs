//! Which history row a Kitsu entry resumes from when more than one
//! maps to it — two providers, two ids, one show.

use super::*;
use crate::app::AppState;
use crate::history::{write_atomic, HistoryEntry};
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::path::PathBuf;
use std::sync::Arc;

fn make_state(history_path: PathBuf) -> AppState {
    state_with(
        history_path,
        crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
    )
}

fn state_with(history_path: PathBuf, kitsu: crate::meta::kitsu::KitsuClient) -> AppState {
    AppState {
        anidb_base: None,
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path,
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu,
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// Two rows for one show: the primary's from an earlier watch, the
/// fallback's from a later one.
fn two_rows_for_one_show(s: &AppState, path: &std::path::Path) {
    write_atomic(
        path,
        &[
            HistoryEntry {
                ep_no: "3".into(),
                id: "the-show-77".into(),
                title: "The Show".into(),
                watched_at: None,
            },
            HistoryEntry {
                ep_no: "7".into(),
                id: "hianime:the-show-9".into(),
                title: "The Show".into(),
                watched_at: None,
            },
        ],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(s, "the-show-77", "K1").unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(s, "hianime:the-show-9", "K1").unwrap();
}

#[test]
fn the_row_watched_last_wins() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    crate::commands::kitsu::watched_at_put(&s, "the-show-77", 1_000).unwrap();
    crate::commands::kitsu::watched_at_put(&s, "hianime:the-show-9", 2_000).unwrap();
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:the-show-9");
    assert_eq!(hit.ep_no, "7");
}

#[test]
fn a_stamped_row_beats_an_unstamped_one() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    crate::commands::kitsu::watched_at_put(&s, "hianime:the-show-9", 2_000).unwrap();
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:the-show-9");
}

/// When the stamps do not separate two rows, progress decides, as
/// it does on the Continue Watching strip: the two surfaces must
/// name one episode, or a play from Home would land over the row
/// the detail page resumes.
#[test]
fn with_no_stamps_the_further_progress_wins() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:the-show-9");
    assert_eq!(hit.ep_no, "7");
}

#[test]
fn with_equal_stamps_the_further_progress_wins() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    crate::commands::kitsu::watched_at_put(&s, "the-show-77", 2_000).unwrap();
    crate::commands::kitsu::watched_at_put(&s, "hianime:the-show-9", 2_000).unwrap();
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:the-show-9");
    assert_eq!(hit.ep_no, "7");
}

/// Two rows for one show on different numberings: the primary's
/// row is a continuation entry the resolver stamped an offset for,
/// so its on-disk `41` is the entry's episode 1; the fallback's row
/// counts from 1, so its `2` is episode 2.
fn two_rows_on_different_numberings(s: &AppState, path: &std::path::Path) {
    write_atomic(
        path,
        &[
            HistoryEntry {
                ep_no: "41".into(),
                id: "the-show-77".into(),
                title: "The Show".into(),
                watched_at: None,
            },
            HistoryEntry {
                ep_no: "2".into(),
                id: "hianime:the-show-9".into(),
                title: "The Show".into(),
                watched_at: None,
            },
        ],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(s, "the-show-77", "K1").unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(s, "hianime:the-show-9", "K1").unwrap();
    crate::commands::anidb_offset::put(s, "the-show-77", 40);
}

/// Progress is compared in the entry's numbering, the one the strip
/// counts in: the primary's `41` is episode 1 once its offset is
/// read, so the fallback's episode 2 is the further progress, and
/// the two surfaces name the same episode.
#[test]
fn with_no_stamps_progress_is_compared_in_the_entrys_numbering() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_on_different_numberings(&s, &path);
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:the-show-9");
    assert_eq!(hit.ep_no, "2");
}

#[test]
fn with_equal_stamps_progress_is_compared_in_the_entrys_numbering() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_on_different_numberings(&s, &path);
    crate::commands::kitsu::watched_at_put(&s, "the-show-77", 2_000).unwrap();
    crate::commands::kitsu::watched_at_put(&s, "hianime:the-show-9", 2_000).unwrap();
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:the-show-9");
    assert_eq!(hit.ep_no, "2");
}

/// A row with no offset stamped reads as it is written — the
/// no-shift case — so it compares by its own number against the
/// other row's translated one, and the winner comes back translated.
#[test]
fn a_row_without_an_offset_compares_by_its_own_number() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[
            HistoryEntry {
                ep_no: "45".into(),
                id: "the-show-77".into(),
                title: "The Show".into(),
                watched_at: None,
            },
            HistoryEntry {
                ep_no: "3".into(),
                id: "hianime:the-show-9".into(),
                title: "The Show".into(),
                watched_at: None,
            },
        ],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "the-show-77", "K1").unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "hianime:the-show-9", "K1").unwrap();
    crate::commands::anidb_offset::put(&s, "the-show-77", 40);
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "the-show-77", "episode 5 is further than episode 3");
    assert_eq!(
        hit.ep_no, "5",
        "the winner comes back in the entry's numbering"
    );
}

/// Equal on every count — no stamps, the same episode — the row
/// first in the file stands, as the strip keeps the row it met
/// first.
#[test]
fn equal_on_every_count_file_order_stands() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[
            HistoryEntry {
                ep_no: "5".into(),
                id: "the-show-77".into(),
                title: "The Show".into(),
                watched_at: None,
            },
            HistoryEntry {
                ep_no: "5".into(),
                id: "hianime:the-show-9".into(),
                title: "The Show".into(),
                watched_at: None,
            },
        ],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "the-show-77", "K1").unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "hianime:the-show-9", "K1").unwrap();
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "the-show-77");
}

/// A row the guard refuses to re-map can still carry a mapping from
/// before the guard existed. Recording its watch stamps it newest,
/// and under that stale mapping it would outrank the row that maps
/// to the entry correctly; the refusal drops the stale mapping
/// instead, and the correctly mapped row is the one to resume.
#[tokio::test]
async fn a_stale_mapping_the_guard_refuses_does_not_outrank_a_correct_row() {
    use wiremock::matchers::{method, path as url_path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    const DETAIL: &[u8] =
        include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(url_path("/anime/12"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(DETAIL.to_vec()),
        )
        .mount(&mock)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = state_with(
        path.clone(),
        crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), mock.uri()),
    );
    write_atomic(
        &path,
        &[
            HistoryEntry {
                ep_no: "7".into(),
                id: "hianime:one-piece-100".into(),
                title: "One Piece".into(),
                watched_at: None,
            },
            HistoryEntry {
                ep_no: "3".into(),
                id: "one-piece-69".into(),
                title: "One Piece Part 2".into(),
                watched_at: None,
            },
        ],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "hianime:one-piece-100", "12").unwrap();
    crate::commands::kitsu::watched_at_put(&s, "hianime:one-piece-100", 1_000).unwrap();
    // The Part 2 row's mapping to the cour-1 entry predates the guard.
    crate::commands::kitsu::allmanga_kitsu_put(&s, "one-piece-69", "12").unwrap();
    let watch = crate::commands::play_native_record::Watch {
        show_id: "one-piece-69".into(),
        title: "One Piece Part 2".into(),
        ep_no: "4".into(),
    };
    crate::commands::play_native_record::record_watch(&s, &watch, Some("12")).await;
    let hit = history_by_kitsu(&s, "12").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:one-piece-100");
    assert_eq!(hit.ep_no, "7");
}

/// Make one cache row unreadable: its `fetched_at` stops being a
/// number, so the reader's row mapping fails for that key alone and
/// every other key still answers. One row must match, or the test
/// would prove nothing about the read it means to break.
fn break_cache_row(s: &AppState, key: &str) {
    let conn = s.cache_pool.get().unwrap();
    let changed = conn
        .execute(
            "UPDATE meta_cache SET fetched_at = 'unreadable' WHERE key = ?1",
            [key],
        )
        .unwrap();
    assert_eq!(changed, 1, "the row to break is at {key}");
}

/// The row watched last has a stamp the cache cannot read. That is
/// not an unstamped row: read as one, its sibling's older stamp
/// would win and the page would resume the stale episode. The
/// function's contract is that cache errors propagate, so the read's
/// failure is the caller's.
#[test]
fn a_failed_stamp_read_is_the_callers_error_not_an_unstamped_row() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    crate::commands::kitsu::watched_at_put(&s, "the-show-77", 1_000).unwrap();
    crate::commands::kitsu::watched_at_put(&s, "hianime:the-show-9", 2_000).unwrap();
    break_cache_row(&s, "watched-at:v1:hianime:the-show-9");
    let got = history_by_kitsu(&s, "K1");
    assert!(
        got.is_err(),
        "a stamp the cache cannot read surfaces as the error it is: {got:?}"
    );
}

/// The cache refuses every write from here on — read-only, or held
/// by another instance — while reads still answer and the history
/// file stays writable, which is the shape of the failure a resume
/// must survive.
fn refuse_cache_writes(s: &AppState) {
    let conn = s.cache_pool.get().unwrap();
    conn.execute_batch("PRAGMA query_only = 1").unwrap();
    drop(conn);
    assert!(
        crate::commands::kitsu::watched_at_put(s, "probe", 1).is_err(),
        "the cache must refuse the write for the case to be the finding's"
    );
}

/// The watch's moment is written beside its row, in the same write,
/// so a cache that refuses the stamp cannot leave the row watched
/// last ranked below its sibling's older stamp: the resume takes the
/// row's own moment when the cache has none, and the strip's stamp
/// map carries it too.
#[tokio::test]
async fn a_watch_the_cache_could_not_stamp_still_resumes_over_an_older_stamped_sibling() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    crate::commands::kitsu::watched_at_put(&s, "the-show-77", 1_000).unwrap();
    refuse_cache_writes(&s);
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let watch = crate::commands::play_native_record::Watch {
        show_id: "hianime:the-show-9".into(),
        title: "The Show".into(),
        ep_no: "2".into(),
    };
    crate::commands::play_native_record::record_watch(&s, &watch, None).await;
    assert_eq!(
        crate::commands::kitsu::watched_at_get(&s, "hianime:the-show-9").unwrap(),
        None,
        "the cache refused the stamp"
    );
    let rows = crate::history::read_all(&path).unwrap();
    let row = rows
        .iter()
        .find(|e| e.id == "hianime:the-show-9")
        .expect("the row");
    assert_eq!(row.ep_no, "2", "the watch reached the file");
    assert!(
        row.watched_at.is_some_and(|ms| ms >= before),
        "the row carries the watch's moment: {:?}",
        row.watched_at
    );
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(hit.id, "hianime:the-show-9", "the row watched last resumes");
    assert_eq!(hit.ep_no, "2");
    let stamps = watched_at_all(&s).unwrap();
    assert_eq!(stamps.get("the-show-77"), Some(&1_000));
    assert_eq!(
        stamps.get("hianime:the-show-9"),
        row.watched_at.as_ref(),
        "the strip's stamps carry the file's moment"
    );
}

/// Where both stores hold a stamp, the later one is the watch: the
/// row's is written with the row, the cache's on mark-watched later.
#[test]
fn the_later_of_the_files_stamp_and_the_caches_is_the_rows_moment() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    write_atomic(
        &path,
        &[
            HistoryEntry {
                ep_no: "3".into(),
                id: "the-show-77".into(),
                title: "The Show".into(),
                watched_at: Some(5_000),
            },
            HistoryEntry {
                ep_no: "7".into(),
                id: "hianime:the-show-9".into(),
                title: "The Show".into(),
                watched_at: Some(2_000),
            },
        ],
    )
    .unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "the-show-77", "K1").unwrap();
    crate::commands::kitsu::allmanga_kitsu_put(&s, "hianime:the-show-9", "K1").unwrap();
    crate::commands::kitsu::watched_at_put(&s, "hianime:the-show-9", 9_000).unwrap();
    let hit = history_by_kitsu(&s, "K1").unwrap().expect("match");
    assert_eq!(
        hit.id, "hianime:the-show-9",
        "the cache's later mark-watched wins"
    );
    let stamps = watched_at_all(&s).unwrap();
    assert_eq!(
        stamps.get("the-show-77"),
        Some(&5_000),
        "the file's stamp alone"
    );
    assert_eq!(
        stamps.get("hianime:the-show-9"),
        Some(&9_000),
        "the later of the two"
    );
}

/// The same for the mapping read: a row whose mapping the cache
/// cannot read is not a row without one, to be skipped for its
/// sibling.
#[test]
fn a_failed_mapping_read_is_the_callers_error_not_a_skipped_row() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history");
    let s = make_state(path.clone());
    two_rows_for_one_show(&s, &path);
    crate::commands::kitsu::watched_at_put(&s, "hianime:the-show-9", 2_000).unwrap();
    break_cache_row(&s, "allmanga2kitsu:v3:hianime:the-show-9");
    let got = history_by_kitsu(&s, "K1");
    assert!(
        got.is_err(),
        "a mapping the cache cannot read surfaces as the error it is: {got:?}"
    );
}
