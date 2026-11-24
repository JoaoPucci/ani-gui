//! How long an anime detail row is kept, by the show's status.
//!
//! The row carries the status and the announced episode count, and an
//! airing show changes both: it finishes, its count gets corrected.
//! Seven days kept a current show's row a week behind; only a finished
//! show's detail is settled enough for that.

use super::tests::state_with_kitsu_at;
use super::*;

const DAY: u64 = 24 * 60 * 60;

fn detail(id: &str, status: Option<&str>) -> KitsuAnimeRef {
    KitsuAnimeRef {
        id: id.into(),
        canonical_title: "Show".into(),
        titles: std::collections::HashMap::new(),
        abbreviated_titles: Vec::new(),
        slug: None,
        synopsis: None,
        start_date: Some("2026-03-19".into()),
        end_date: None,
        episode_count: Some(12),
        average_rating: None,
        subtype: Some("ONA".into()),
        status: status.map(str::to_string),
        age_rating: None,
        popularity_rank: None,
        poster_image: None,
        anilist_banner: None,
        cover_image: Some(KitsuCoverImage {
            tiny: None,
            small: None,
            large: None,
            original: None,
        }),
    }
}

fn stored_ttl(state: &AppState, id: &str) -> u64 {
    let conn = state.cache_pool.get().expect("conn");
    let ttl: i64 = conn
        .query_row(
            "SELECT ttl_seconds FROM meta_cache WHERE key = ?1",
            [anime_detail_key(id)],
            |r| r.get(0),
        )
        .expect("row written");
    u64::try_from(ttl).expect("non-negative")
}

#[test]
fn a_finished_show_keeps_its_week() {
    assert_eq!(anime_detail_ttl(Some("finished")), 7 * DAY);
}

#[test]
fn a_show_still_airing_or_to_come_is_kept_a_day() {
    for status in ["current", "upcoming", "unreleased", "tba"] {
        assert_eq!(anime_detail_ttl(Some(status)), DAY, "{status}");
    }
    assert_eq!(anime_detail_ttl(None), DAY);
}

#[test]
fn the_warm_writes_a_current_show_for_a_day() {
    let state = state_with_kitsu_at("http://127.0.0.1:1");
    warm_anime_detail_cache(&state, &detail("1", Some("current")));
    warm_anime_detail_cache(&state, &detail("2", Some("finished")));
    assert_eq!(stored_ttl(&state, "1"), DAY);
    assert_eq!(stored_ttl(&state, "2"), 7 * DAY);
}

proptest::proptest! {
    /// Only the exact `finished` status earns the week: any other
    /// string Kitsu might send, or none, is kept a day.
    #[test]
    fn only_a_finished_show_keeps_the_week(status in proptest::option::of("\\PC{0,12}")) {
        let ttl = anime_detail_ttl(status.as_deref());
        if status.as_deref() == Some("finished") {
            proptest::prop_assert_eq!(ttl, 7 * DAY);
        } else {
            proptest::prop_assert_eq!(ttl, DAY);
        }
    }
}

// — A show's episode pages follow its status too ————————————————————
//
// The pages carry no status of their own; the show's detail row does,
// when one is held as the page is written (nothing orders the detail
// read ahead of the episodes). A page is kept a week only while that
// row is held, says the show finished, and the show has settled, a
// week past its end date: an airing, upcoming or unannounced show's
// page, a just-finished one's, and one written with no detail held to
// say, is kept a day.

const EPISODES_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/episodes_one_piece.json");

async fn kitsu_with_episodes() -> wiremock::MockServer {
    let mock = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/anime/12/episodes"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(EPISODES_FIXTURE.to_vec()),
        )
        .mount(&mock)
        .await;
    mock
}

/// The show's detail held for `ttl`, ended long enough ago to have
/// settled (`show_settled`).
fn hold_detail(state: &AppState, status: Option<&str>, ttl: u64) {
    hold_detail_ended(state, status, Some("2026-06-01"), ttl);
}

fn hold_detail_ended(state: &AppState, status: Option<&str>, end: Option<&str>, ttl: u64) {
    let mut held = detail("12", status);
    held.end_date = end.map(str::to_string);
    let body = serde_json::to_string(&held).expect("json");
    meta_cache_put(&state.cache_pool, &anime_detail_key("12"), &body, ttl).expect("seed");
}

fn page_ttl(state: &AppState, page: u32) -> u64 {
    stored_ttl_at(state, &format!("kitsu:episodes:12:p{page}"))
}

fn stored_ttl_at(state: &AppState, key: &str) -> u64 {
    let conn = state.cache_pool.get().expect("conn");
    let ttl: i64 = conn
        .query_row(
            "SELECT ttl_seconds FROM meta_cache WHERE key = ?1",
            [key],
            |r| r.get(0),
        )
        .expect("row written");
    u64::try_from(ttl).expect("non-negative")
}

#[tokio::test]
async fn a_finished_show_s_episode_pages_keep_the_week() {
    let mock = kitsu_with_episodes().await;
    let state = state_with_kitsu_at(&mock.uri());
    hold_detail(&state, Some("finished"), 7 * DAY);
    kitsu_episode_page(&state, "12", 1).await.expect("page 1");
    kitsu_episode_page(&state, "12", 2).await.expect("page 2");
    assert_eq!(page_ttl(&state, 1), 7 * DAY);
    assert_eq!(page_ttl(&state, 2), 7 * DAY);
}

#[tokio::test]
async fn a_refreshed_page_of_a_finished_show_keeps_the_week() {
    let mock = kitsu_with_episodes().await;
    let state = state_with_kitsu_at(&mock.uri());
    hold_detail(&state, Some("finished"), 7 * DAY);
    // Every row of the fixture carries a still, so the route asks
    // AniList for nothing.
    meta_cache_put(&state.cache_pool, "kitsu:episodes:12:p1", "[]", 60).expect("seed");
    kitsu_episodes_refresh(&state, "12", 1)
        .await
        .expect("refresh");
    assert_eq!(page_ttl(&state, 1), 7 * DAY);
}

#[tokio::test]
async fn an_airing_upcoming_or_unannounced_show_s_pages_are_kept_a_day() {
    for status in [
        Some("current"),
        Some("upcoming"),
        Some("unreleased"),
        Some("tba"),
        None,
    ] {
        let mock = kitsu_with_episodes().await;
        let state = state_with_kitsu_at(&mock.uri());
        hold_detail(&state, status, DAY);
        kitsu_episode_page(&state, "12", 1).await.expect("page");
        assert_eq!(page_ttl(&state, 1), DAY, "{status:?}");
    }
}

#[tokio::test]
async fn a_page_written_with_no_detail_held_is_kept_a_day() {
    let mock = kitsu_with_episodes().await;
    let state = state_with_kitsu_at(&mock.uri());
    kitsu_episode_page(&state, "12", 1).await.expect("page");
    assert_eq!(page_ttl(&state, 1), DAY);
}

#[tokio::test]
async fn a_finished_detail_past_its_lifetime_says_nothing() {
    // An expired row is not served, so it is not read for its status
    // either: the page is written as for a show of unknown status.
    let mock = kitsu_with_episodes().await;
    let state = state_with_kitsu_at(&mock.uri());
    hold_detail(&state, Some("finished"), 0);
    kitsu_episode_page(&state, "12", 1).await.expect("page");
    assert_eq!(page_ttl(&state, 1), DAY);
}

#[tokio::test]
async fn a_detail_row_that_cannot_be_read_says_nothing() {
    let mock = kitsu_with_episodes().await;
    let state = state_with_kitsu_at(&mock.uri());
    meta_cache_put(&state.cache_pool, &anime_detail_key("12"), "{", 7 * DAY).expect("seed");
    kitsu_episode_page(&state, "12", 1).await.expect("page");
    assert_eq!(page_ttl(&state, 1), DAY);
}

#[tokio::test]
async fn a_show_that_finishes_while_its_page_is_held_gets_the_week_on_the_next_write() {
    let mock = kitsu_with_episodes().await;
    let state = state_with_kitsu_at(&mock.uri());
    hold_detail(&state, Some("current"), DAY);
    kitsu_episode_page(&state, "12", 1).await.expect("page");
    assert_eq!(page_ttl(&state, 1), DAY);
    // The day passes; the detail is read again and says finished.
    hold_detail(&state, Some("finished"), 7 * DAY);
    crate::cache::meta_cache_delete(&state.cache_pool, "kitsu:episodes:12:p1").expect("expire");
    kitsu_episode_page(&state, "12", 1).await.expect("page");
    assert_eq!(page_ttl(&state, 1), 7 * DAY);
}

#[tokio::test]
async fn a_show_that_has_only_just_finished_keeps_its_pages_a_day() {
    // Kitsu calls a show finished from the day after its end date, and
    // its last episodes' titles and stills often come in after that: a
    // page written before the show has settled is asked again daily.
    for end in [
        Some("2999-01-01"),
        None,
        Some("2026-06"),
        Some("not a date"),
    ] {
        let mock = kitsu_with_episodes().await;
        let state = state_with_kitsu_at(&mock.uri());
        hold_detail_ended(&state, Some("finished"), end, 7 * DAY);
        kitsu_episode_page(&state, "12", 1).await.expect("page");
        assert_eq!(page_ttl(&state, 1), DAY, "{end:?}");
    }
}
