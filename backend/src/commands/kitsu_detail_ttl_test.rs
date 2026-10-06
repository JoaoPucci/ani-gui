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
