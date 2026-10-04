//! A play begun before its show was removed from history writes
//! nothing of the show afterwards.
//!
//! A play waits on the network — the provider's pages, the CDN's
//! answer for a cached stream — and writes when it has its stream: the
//! history row, the show's numbering, the resolution row. The user can
//! leave the page and remove the show from history while it waits, and
//! what the play then wrote would outlive the removal. A removal wins
//! over a play begun before it; a play begun after it is new.

use super::tests::{cached_blank, state_with_proxy_origin};
use super::*;
use crate::commands::play_native_record::{stamp_numbering, write_history};
use crate::history::guard::epoch;

const SHOW: &str = "the-show-77";
const KEY: &str = "play:v14:The Show:sub:best:1:::";

fn state_in(dir: &std::path::Path) -> AppState {
    let mut state = state_with_proxy_origin();
    state.history_path = dir.join("history");
    state
}

fn seed_row(state: &AppState, id: &str) {
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: id.into(),
            title: "The Show".into(),
            watched_at: None,
            kitsu_id: None,
        },
    )
    .expect("seed row");
}

fn native() -> NativeResolved {
    NativeResolved {
        slug: SHOW.into(),
        title: "The Show".into(),
        master_url: "https://cdn.example/x/master.m3u8".into(),
        episode_cap: Some(3),
        numbering_offset: 12,
        extra_tags: Vec::new(),
        resolved_slot: 2,
        resolved_tag: None,
        provider: crate::scraper::provider::ProviderId::Anidb,
        referer: None,
        subtitles: Vec::new(),
    }
}

fn cached() -> CachedResolution {
    let mut cached = cached_blank(
        "https://cdn.example/x/master.m3u8".into(),
        String::new(),
        MediaKind::Hls,
    );
    cached.show_id = SHOW.into();
    cached.show_title = "The Show".into();
    cached.resolved_slot = Some(2);
    cached
}

fn args() -> PlayArgs {
    PlayArgs {
        title: "The Show".into(),
        episode: "2".into(),
        mode: "sub".into(),
        quality: None,
        subtype: None,
        episode_count: None,
        year: None,
        alt_titles: vec![],
        prefetch: false,
        kitsu_id: None,
    }
}

/// Everything a play writes once it has its stream, for a play that
/// began at `begun`: the row a fresh resolve writes, the row a cached
/// stream writes, the show's numbering, and the resolution row.
fn finish_play(state: &AppState, begun: crate::history::guard::Epoch) {
    finish_asked(state, crate::history::guard::Asked { begun, page: None });
}

/// [`finish_play`] for a play asked for from a Kitsu page.
fn finish_asked(state: &AppState, asked: crate::history::guard::Asked<'_>) {
    write_history(state, &native(), "2", asked);
    write_history_on_cache_hit(state, &args(), &cached(), asked);
    stamp_numbering(state, &native(), asked);
    play_resolution_cache::store(state, asked, KEY, &cached());
}

/// What a finished play left of the show: its row, its numbering, its
/// resolution row.
fn left(state: &AppState) -> (usize, u32, bool) {
    (
        crate::history::read_all(&state.history_path)
            .expect("rows")
            .iter()
            .filter(|r| r.id == SHOW)
            .count(),
        crate::commands::anidb_offset::get(state, SHOW),
        crate::cache::meta_cache_get(&state.cache_pool, KEY)
            .expect("cache")
            .is_some(),
    )
}

#[test]
fn a_play_begun_before_a_delete_writes_nothing_of_the_show() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    seed_row(&state, SHOW);
    let begun = epoch(&state.history_path);

    assert!(crate::commands::history::history_delete(&state, SHOW).unwrap());
    finish_play(&state, begun);

    assert_eq!(left(&state), (0, 0, false));
}

#[test]
fn a_play_begun_before_a_clear_writes_nothing_of_the_show() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    seed_row(&state, SHOW);
    let begun = epoch(&state.history_path);

    crate::commands::history::history_clear(&state).unwrap();
    finish_play(&state, begun);

    assert_eq!(left(&state), (0, 0, false));
}

/// A play begun after the removal is the user playing the show again:
/// it records as any play does.
#[test]
fn a_play_begun_after_the_removal_records_as_any_play() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    seed_row(&state, SHOW);
    assert!(crate::commands::history::history_delete(&state, SHOW).unwrap());

    finish_play(&state, epoch(&state.history_path));

    assert_eq!(left(&state), (1, 12, true));
}

/// Only the removed show's plays are affected.
#[test]
fn removing_another_show_leaves_a_pending_play_whole() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    seed_row(&state, "another-show-5");
    let begun = epoch(&state.history_path);

    assert!(crate::commands::history::history_delete(&state, "another-show-5").unwrap());
    finish_play(&state, begun);

    assert_eq!(left(&state), (1, 12, true));
}

/// A row for the show under another provider's key, recording the
/// Kitsu page it was played from.
fn seed_row_of_page(state: &AppState, id: &str, page: &str) {
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: id.into(),
            title: "The Show".into(),
            watched_at: None,
            kitsu_id: Some(page.into()),
        },
    )
    .expect("seed row");
}

/// A play resolves its show under the key of whichever provider
/// answers — another provider's when the walk fails over. Removing the
/// show's row, keyed as it was played before, has to stop the play's
/// writes under the new key too: the play was asked for from the
/// show's Kitsu page, and the removed row was known by that page.
#[test]
fn a_play_that_resolves_under_another_key_loses_to_the_removal_too() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    seed_row_of_page(&state, "hianime:the-show-100", "77");
    let asked = crate::history::guard::Asked::now(&state.history_path, Some("77"));

    assert!(crate::commands::history::history_delete(&state, "hianime:the-show-100").unwrap());
    finish_asked(&state, asked);

    assert_eq!(left(&state), (0, 0, false));
}

/// While another row still claims the page, the show is still in
/// history, and a play of it records under whatever key it resolved.
#[test]
fn a_page_a_remaining_row_still_claims_keeps_its_play() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    seed_row_of_page(&state, "hianime:the-show-100", "77");
    seed_row_of_page(&state, "the-show-old-3", "77");
    let asked = crate::history::guard::Asked::now(&state.history_path, Some("77"));

    assert!(crate::commands::history::history_delete(&state, "hianime:the-show-100").unwrap());
    finish_asked(&state, asked);

    assert_eq!(left(&state), (1, 12, true));
}

/// The row a play writes does not record its Kitsu page — the watch's
/// verdict does that later — but a removal has to know the page: the
/// skip times the player fetched are cached under it.
#[test]
fn removing_a_row_a_play_just_wrote_takes_the_skip_times_of_its_page() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    finish_asked(
        &state,
        crate::history::guard::Asked::now(&state.history_path, Some("77")),
    );
    crate::cache::meta_cache_put(&state.cache_pool, "aniskip:v2:77:5:2", "[]", 3600).unwrap();

    assert!(crate::commands::history::history_delete(&state, SHOW).unwrap());

    assert_eq!(
        crate::cache::meta_cache_get(&state.cache_pool, "aniskip:v2:77:5:2").unwrap(),
        None
    );
}

/// A page is noted for a row that was written. A play whose row could
/// not be written leaves no page behind for a later row under the same
/// key to answer for: that row's removal takes nothing of a page it
/// was never played from.
#[test]
fn a_row_that_failed_to_write_leaves_no_page_behind() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    // Block the history's atomic write, as an unwritable state
    // directory would.
    std::fs::create_dir(tmp.path().join("history.new")).unwrap();
    write_history(
        &state,
        &native(),
        "2",
        crate::history::guard::Asked::now(&state.history_path, Some("77")),
    );
    std::fs::remove_dir(tmp.path().join("history.new")).unwrap();
    write_history(
        &state,
        &native(),
        "2",
        crate::history::guard::Asked::now(&state.history_path, None),
    );
    crate::cache::meta_cache_put(&state.cache_pool, "aniskip:v2:77:5:2", "[]", 3600).unwrap();

    assert!(crate::commands::history::history_delete(&state, SHOW).unwrap());

    assert!(
        crate::cache::meta_cache_get(&state.cache_pool, "aniskip:v2:77:5:2")
            .unwrap()
            .is_some(),
        "the removed row was never played from that page"
    );
}

// — what a show's page resolved under another key ——————————————————
//
// A page warms its episodes whether or not the show is in history, and
// a warm that fails over resolves under the other provider's key: a
// resolution row and a numbering for a key the show's history row does
// not have. A removal finds those by the page they were resolved from.

fn resolution(state: &AppState, key: &str) -> Option<CachedResolution> {
    play_resolution_cache::get(&state.cache_pool, key).expect("cache")
}

#[test]
fn a_resolution_row_records_the_page_it_was_resolved_from() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());

    finish_asked(
        &state,
        crate::history::guard::Asked::now(&state.history_path, Some("77")),
    );

    assert_eq!(
        resolution(&state, KEY).expect("stored").kitsu_id.as_deref(),
        Some("77")
    );
}

#[test]
fn removing_a_show_takes_what_its_page_resolved_under_another_key() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    seed_row_of_page(&state, "hianime:the-show-100", "77");
    // A warm of the show's page that resolved under the other key: a
    // resolution row and a numbering, and no history row.
    let warm = crate::history::guard::Asked::now(&state.history_path, Some("77"));
    stamp_numbering(&state, &native(), warm);
    play_resolution_cache::store(&state, warm, KEY, &cached());
    // Another page's warm, which the removal has no business with.
    let other_key = "play:v14:Other Show:sub:best:1:::";
    let mut other = cached();
    other.show_id = "other-show-9".into();
    play_resolution_cache::store(
        &state,
        crate::history::guard::Asked::now(&state.history_path, Some("88")),
        other_key,
        &other,
    );
    crate::commands::anidb_offset::put(&state, "other-show-9", 3);

    assert!(crate::commands::history::history_delete(&state, "hianime:the-show-100").unwrap());

    assert_eq!(left(&state), (0, 0, false), "the show's, under either key");
    assert!(
        resolution(&state, other_key).is_some(),
        "another page's stays"
    );
    assert_eq!(
        crate::commands::anidb_offset::get(&state, "other-show-9"),
        3
    );
}

/// A key with a history row of its own keeps its numbering: the row
/// needs it to be read, whatever page its resolution rows name.
#[test]
fn a_key_with_a_row_of_its_own_keeps_its_numbering() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    seed_row_of_page(&state, "hianime:the-show-100", "77");
    // A row under the other key that the history never linked to the
    // page, with the numbering and resolution row a play left it.
    seed_row(&state, SHOW);
    let play = crate::history::guard::Asked::now(&state.history_path, Some("77"));
    stamp_numbering(&state, &native(), play);
    play_resolution_cache::store(&state, play, KEY, &cached());

    assert!(crate::commands::history::history_delete(&state, "hianime:the-show-100").unwrap());

    let (rows, offset, _) = left(&state);
    assert_eq!(
        (rows, offset),
        (1, 12),
        "the remaining row and its numbering"
    );
}

/// An empty id names no page. A removed row whose title match was
/// stored with an empty id is not thereby known by "the page" of every
/// resolution row that records none.
#[test]
fn a_resolution_row_with_no_page_is_no_removed_shows() {
    let tmp = tempfile::tempdir().unwrap();
    let state = state_in(tmp.path());
    seed_row(&state, "hianime:the-show-100");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "title-match:v3:hianime:the show:c1",
        "",
        3600,
    )
    .unwrap();
    let other_key = "play:v14:Other Show:sub:best:1:::";
    let mut other = cached();
    other.show_id = "other-show-9".into();
    play_resolution_cache::put(&state.cache_pool, other_key, &other);

    assert!(crate::commands::history::history_delete(&state, "hianime:the-show-100").unwrap());

    assert!(resolution(&state, other_key).is_some());
}
