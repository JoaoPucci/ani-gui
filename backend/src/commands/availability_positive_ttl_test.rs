//! The positive row an ongoing show is written with expires at its
//! next airing, not a flat day later. Mounted by `#[path]` beside the
//! availability tests and borrowing their state builder.

use super::tests::cache_only_state;
use super::*;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn positive(count: u32) -> AvailabilityResponse {
    AvailabilityResponse {
        available: true,
        episode_count: Some(count),
        extra_episodes: Vec::new(),
        episode_count_approximate: false,
        gate_refused: false,
        provider: None,
    }
}

fn seed_next_airing(state: &AppState, kitsu_id: &str, at: u64) {
    let body = format!(r#"{{"aired":7,"next_episode":8,"next_airing_at":{at},"upcoming":[]}}"#);
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &format!("airing:v2:{kitsu_id}"),
        &body,
        3600,
    )
    .expect("seed airing row");
}

fn stored_ttl(state: &AppState, kitsu_id: &str) -> u64 {
    let conn = state.cache_pool.get().expect("conn");
    let ttl: i64 = conn
        .query_row(
            "SELECT ttl_seconds FROM meta_cache WHERE key = ?1",
            [cache_key(kitsu_id, "sub")],
            |r| r.get(0),
        )
        .expect("row written");
    u64::try_from(ttl).expect("non-negative ttl")
}

#[test]
fn a_weekly_show_s_count_expires_after_its_next_episode_drops() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    seed_next_airing(&state, "weekly", now() + 2 * 60 * 60);
    write_cache_full(&state, "weekly", "sub", Some("current"), &positive(7));
    let ttl = stored_ttl(&state, "weekly");
    // Two hours to the drop plus the provider's grace, not a day.
    assert!(ttl <= 3 * 60 * 60, "ttl {ttl}");
    assert!(ttl > 2 * 60 * 60, "ttl {ttl}");
}

#[test]
fn a_show_with_no_schedule_keeps_the_ongoing_day() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    write_cache_full(&state, "ona", "sub", Some("current"), &positive(12));
    assert_eq!(stored_ttl(&state, "ona"), AVAILABILITY_TTL_ONGOING_SECS);
}

#[test]
fn a_finished_show_keeps_its_month() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    seed_next_airing(&state, "finished", now() + 2 * 60 * 60);
    write_cache_full(&state, "finished", "sub", Some("finished"), &positive(12));
    assert_eq!(
        stored_ttl(&state, "finished"),
        AVAILABILITY_TTL_FINISHED_SECS
    );
}

fn seed_schedule(state: &AppState, kitsu_id: &str, aired: u32, at: u64) {
    let body = format!(
        r#"{{"aired":{aired},"next_episode":{},"next_airing_at":{at},"upcoming":[]}}"#,
        aired + 1
    );
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &format!("airing:v2:{kitsu_id}"),
        &body,
        3600,
    )
    .expect("seed airing row");
}

#[test]
fn a_count_written_behind_the_schedule_is_re_asked_within_the_hour() {
    // Just past a drop: AniList already points at next week's episode
    // 9, the provider still lists 7.
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    seed_schedule(&state, "behind", 8, now() + 6 * 24 * 60 * 60);
    write_cache_full(&state, "behind", "sub", Some("current"), &positive(7));
    assert!(stored_ttl(&state, "behind") <= 60 * 60);
}

#[test]
fn a_count_level_with_the_schedule_keeps_its_day() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    seed_schedule(&state, "level", 8, now() + 6 * 24 * 60 * 60);
    write_cache_full(&state, "level", "sub", Some("current"), &positive(8));
    assert_eq!(stored_ttl(&state, "level"), AVAILABILITY_TTL_ONGOING_SECS);
}

#[test]
fn a_finished_show_behind_its_total_keeps_its_month() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    seed_schedule(&state, "done-behind", 12, now() + 6 * 24 * 60 * 60);
    write_cache_full(
        &state,
        "done-behind",
        "sub",
        Some("finished"),
        &positive(11),
    );
    assert_eq!(
        stored_ttl(&state, "done-behind"),
        AVAILABILITY_TTL_FINISHED_SECS
    );
}

fn warm_args(kitsu_id: &str, status: &str) -> AvailabilityArgs {
    serde_json::from_value(serde_json::json!({
        "title": kitsu_id,
        "mode": "sub",
        "kitsu_id": kitsu_id,
        "status": status,
    }))
    .expect("args")
}

#[test]
fn the_warm_seeds_schedules_for_shows_on_air_as_well_as_to_come() {
    // A current show's positive row is bounded by its next airing only
    // if the schedule is cached before the warm's probe writes it.
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    write_cache_full(&state, "fresh", "sub", Some("current"), &positive(3));
    let items = [
        warm_args("airing", "current"),
        warm_args("premiere", "upcoming"),
        warm_args("done", "finished"),
        warm_args("fresh", "current"),
    ];
    assert_eq!(
        schedule_seed_ids(&state, &items),
        vec!["airing".to_string(), "premiere".to_string()]
    );
}
