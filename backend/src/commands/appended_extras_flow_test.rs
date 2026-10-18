//! Every caller that picks a show reads the extras a provider appends
//! after its last episode the same way: availability, the embedded
//! player, the external-player and Syncplay handoff, a single download
//! and a range download. They all stamp the availability row with the
//! cap and extras their own resolve saw, so one caller reading
//! Noragami's OADs as episodes 13 and 14 would put them back.
//!
//! The show is the recorded one: hianime's Noragami listing, Kitsu's
//! entry and episode page for it, and AniList's answer, all served from
//! the cache the way a detail page visit leaves them. Kitsu itself is
//! unreachable, so nothing here asks the network.

use crate::commands::availability::{cache_key, check_availability, AvailabilityArgs};
use crate::scraper::provider::{BrowseHit, EpisodeRef, ProviderId};
use crate::scraper::table_provider::{Asked, TableProvider};
use std::sync::Arc;

const KITSU: &str = "7881";

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/hianime/appended")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn noragami_rows() -> Vec<EpisodeRef> {
    let body = String::from_utf8(fixture("episode-list-916.json")).expect("utf-8");
    crate::scraper::hianime::parse_episode_list(&body).expect("recorded listing")
}

fn master(id: u64) -> String {
    format!("https://cdn.example/m/{id}/master.m3u8")
}

/// hianime as recorded for Noragami: every search finds the show,
/// every row answers sub, and each row streams from a master of its
/// own.
fn hianime() -> Arc<TableProvider> {
    let rows = noragami_rows();
    let table = TableProvider::new()
        .answering_as(ProviderId::TestA)
        .search_default(vec![BrowseHit {
            slug: "noragami-916".into(),
            title: "Noragami".into(),
            kind: Some("TV".into()),
            jname: Some("Noragami".into()),
        }])
        .listing(916, rows.clone())
        .modes_default(true);
    Arc::new(
        rows.iter()
            .fold(table, |t, row| t.stream(row.id, "sub", &master(row.id))),
    )
}

/// The OAD hianime lists as row 13.
fn first_oad() -> u64 {
    noragami_rows()[12].id
}

fn state_over(td: &tempfile::TempDir, provider: &Arc<TableProvider>) -> crate::app::AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    crate::app::AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: td.path().join("history"),
        providers: crate::provider_set::ProviderSet::for_tests().with_stub(Arc::clone(provider)),
        image_cache_dir: td.path().join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: td.path().join("config.toml"),
        state_dir: td.path().join("state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// The cache as a visit to Noragami's detail page leaves it — Kitsu's
/// entry, its first episode page, and AniList's airing answer, with
/// Kitsu's status replaced by `status` — and as a first read of its
/// evidence leaves it: the entries Kitsu relates it to, and its TV
/// sequel's first episode page.
fn seed_kitsu(state: &crate::app::AppState, status: &str) {
    use crate::cache::meta_cache_put;
    let mut detail =
        crate::meta::kitsu::parse_anime_response(&fixture("kitsu-anime-7881.json")).expect("entry");
    detail.status = Some(status.to_string());
    let page = crate::meta::kitsu::parse_episodes_response(&fixture("kitsu-episodes-7881-p1.json"))
        .expect("page");
    let airing =
        crate::meta::anilist_airing::parse_airing_response(&fixture("anilist-airing-7881.json"))
            .expect("airing")
            .expect("known");
    let day = 24 * 60 * 60;
    meta_cache_put(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key(KITSU),
        &serde_json::to_string(&detail).expect("json"),
        day,
    )
    .expect("detail row");
    meta_cache_put(
        &state.cache_pool,
        &format!("kitsu:episodes:{KITSU}:p1"),
        &serde_json::to_string(&page).expect("json"),
        day,
    )
    .expect("page row");
    // The page after, which Kitsu answers empty for a twelve-episode
    // show.
    meta_cache_put(
        &state.cache_pool,
        &format!("kitsu:episodes:{KITSU}:p2"),
        "[]",
        day,
    )
    .expect("page after");
    let relations =
        crate::meta::kitsu::parse_relationships_response(&fixture("kitsu-relations-7881.json"))
            .expect("relations");
    meta_cache_put(
        &state.cache_pool,
        &format!("kitsu:relations:v1:{KITSU}"),
        &serde_json::to_string(&relations).expect("json"),
        day,
    )
    .expect("relations row");
    // The sequel's first page, which the rows past the count are
    // checked against.
    let sequel =
        crate::meta::kitsu::parse_episodes_response(&fixture("kitsu-episodes-10800-p1.json"))
            .expect("sequel page");
    meta_cache_put(
        &state.cache_pool,
        "kitsu:episodes:10800:p1",
        &serde_json::to_string(&sequel).expect("json"),
        day,
    )
    .expect("sequel page row");
    meta_cache_put(
        &state.cache_pool,
        &format!("airing:v2:{KITSU}"),
        &serde_json::to_string(&airing).expect("json"),
        day,
    )
    .expect("airing row");
}

fn availability_args() -> AvailabilityArgs {
    serde_json::from_value(serde_json::json!({
        "title": "Noragami",
        "mode": "sub",
        "episode_count": 12,
        "kitsu_id": KITSU,
        "status": "finished",
    }))
    .expect("args")
}

/// The cap and extras the availability row holds.
fn stamped(state: &crate::app::AppState) -> (Option<u32>, Vec<String>) {
    let body = crate::cache::meta_cache_get(&state.cache_pool, &cache_key(KITSU, "sub"))
        .expect("read")
        .expect("a row");
    let row: crate::commands::availability::AvailabilityResponse =
        serde_json::from_str(&body).expect("row");
    (row.episode_count, row.extra_episodes)
}

fn oads() -> Vec<String> {
    vec!["12.1".to_string(), "12.2".to_string()]
}

#[tokio::test]
async fn availability_caps_noragami_at_twelve_with_its_oads_as_extras() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "finished");
    let got = check_availability(&state, &availability_args())
        .await
        .expect("answered");
    assert!(got.available);
    assert_eq!(got.episode_count, Some(12));
    assert_eq!(got.extra_episodes, oads());
    assert_eq!(stamped(&state), (Some(12), oads()));
}

/// While Kitsu calls the show anything but finished, the provider may
/// simply be ahead of it: the listing stays as it is.
#[tokio::test]
async fn availability_leaves_the_listing_of_a_show_kitsu_calls_airing() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "current");
    let got = check_availability(&state, &availability_args())
        .await
        .expect("answered");
    assert_eq!(got.episode_count, Some(14));
    assert!(got.extra_episodes.is_empty());
}

fn play_args(episode: &str) -> crate::commands::play::PlayArgs {
    serde_json::from_value(serde_json::json!({
        "title": "Noragami",
        "episode": episode,
        "mode": "sub",
        "episode_count": 12,
        "kitsu_id": KITSU,
    }))
    .expect("args")
}

fn mastered(provider: &TableProvider) -> Vec<u64> {
    provider
        .asked()
        .into_iter()
        .filter_map(|a| match a {
            Asked::Master(id, _) => Some(id),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn the_player_plays_the_first_oad_as_12_1_and_stamps_the_cap_at_twelve() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "finished");
    crate::commands::play::play_with_progress(&state, &play_args("12.1"), |_| {})
        .await
        .expect("the OAD plays");
    assert_eq!(mastered(&provider), vec![first_oad()]);
    assert_eq!(stamped(&state), (Some(12), oads()));
}

#[tokio::test]
async fn the_player_no_longer_offers_the_first_oad_as_episode_13() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "finished");
    let got = crate::commands::play::play_with_progress(&state, &play_args("13"), |_| {}).await;
    assert!(
        matches!(got, Err(crate::error::AniError::EpisodeUnavailable)),
        "{got:?}"
    );
    assert!(mastered(&provider).is_empty());
}

#[tokio::test]
async fn the_handoff_hands_the_first_oad_over_as_12_1() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "finished");
    let args = play_args("12.1");
    let asked = crate::history::guard::Asked::now(&state.history_path, args.kitsu_id.as_deref());
    let (launch, _) = crate::commands::play_handoff::resolve_launch_args(&state, &args, asked)
        .await
        .expect("the OAD resolves");
    assert_eq!(launch.stream_url, master(first_oad()));
    assert_eq!(stamped(&state), (Some(12), oads()));
}

#[cfg(unix)]
fn recording_tool(bin: &std::path::Path, log: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let p = bin.join("yt-dlp");
    std::fs::write(
        &p,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\nprev=\"\"\nfor a in \"$@\"; do if [ \"$prev\" = \"-o\" ]; then printf 'video' > \"$a\"; fi; prev=\"$a\"; done\nexit 0\n",
            log.display()
        ),
    )
    .expect("write tool");
    let mut perms = std::fs::metadata(&p).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&p, perms).expect("chmod");
}

#[cfg(unix)]
fn download_args(episode: &str, dest: &std::path::Path) -> crate::commands::download::DownloadArgs {
    serde_json::from_value(serde_json::json!({
        "title": "Noragami",
        "episode": episode,
        "mode": "sub",
        "episode_count": 12,
        "kitsu_id": KITSU,
        "download_dir": dest.to_string_lossy(),
    }))
    .expect("args")
}

#[cfg(unix)]
#[tokio::test]
async fn a_download_of_12_1_transfers_the_first_oad() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "finished");
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let log = dest.path().join("calls.log");
    recording_tool(bin.path(), &log);
    crate::commands::download::download_with_tools(
        &state,
        &download_args("12.1", dest.path()),
        &bin.path().display().to_string(),
        |_| {},
    )
    .await
    .expect("downloaded");
    let calls = std::fs::read_to_string(&log).expect("the tool ran");
    assert!(calls.contains(&master(first_oad())), "{calls}");
    assert_eq!(stamped(&state), (Some(12), oads()));
}

/// A range past the last episode stops there: episode 12 transfers,
/// and the OAD is not episode 13.
#[cfg(unix)]
#[tokio::test]
async fn a_range_download_ends_at_the_last_episode() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "finished");
    let bin = tempfile::tempdir().expect("bin");
    let dest = tempfile::tempdir().expect("dest");
    let log = dest.path().join("calls.log");
    recording_tool(bin.path(), &log);
    let got = crate::commands::download::download_with_tools(
        &state,
        &download_args("12-13", dest.path()),
        &bin.path().display().to_string(),
        |_| {},
    )
    .await;
    assert!(
        matches!(got, Err(crate::error::AniError::EpisodeUnavailable)),
        "{got:?}"
    );
    let calls = std::fs::read_to_string(&log).expect("the tool ran");
    assert_eq!(calls.lines().count(), 1, "{calls}");
    assert!(calls.contains(&master(noragami_rows()[11].id)), "{calls}");
    assert!(!calls.contains(&master(first_oad())), "{calls}");
}

/// Once availability proved the reading, a play whose evidence cannot
/// be read — AniList's answer gone from the cache and Kitsu
/// unreachable — reads the listing the same way: it stamps twelve with
/// the two extras, not the listing's fourteen, and the OAD still plays
/// as `12.1`.
#[tokio::test]
async fn a_play_without_its_evidence_keeps_the_reading_availability_proved() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "finished");
    check_availability(&state, &availability_args())
        .await
        .expect("answered");
    crate::cache::meta_cache_delete(&state.cache_pool, &format!("airing:v2:{KITSU}"))
        .expect("airing row gone");
    crate::commands::play::play_with_progress(&state, &play_args("5"), |_| {})
        .await
        .expect("episode 5 plays");
    assert_eq!(stamped(&state), (Some(12), oads()));
    crate::commands::play::play_with_progress(&state, &play_args("12.1"), |_| {})
        .await
        .expect("the OAD plays");
    assert_eq!(mastered(&provider).last().copied(), Some(first_oad()));
    assert_eq!(stamped(&state), (Some(12), oads()));
}

/// A history row an earlier version wrote — Noragami's first OAD
/// recorded as episode 13, with no numbering stamp to translate it —
/// reads back as `12.1` once availability has proved the reading, so
/// Continue resumes the OAD instead of offering an episode 13 nothing
/// plays.
#[tokio::test]
async fn an_oad_an_earlier_version_recorded_as_episode_13_reads_back_as_12_1() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    let row = crate::history::HistoryEntry {
        ep_no: "13".into(),
        id: "hianime:noragami-916".into(),
        title: "Noragami".into(),
        watched_at: None,
        kitsu_id: Some(KITSU.into()),
        own_number: false,
    };
    crate::history::write_atomic(&state.history_path, std::slice::from_ref(&row)).expect("history");
    let read = |state: &crate::app::AppState| {
        crate::commands::history::history_list(state).expect("history")[0]
            .ep_no
            .clone()
    };
    assert_eq!(read(&state), "13", "nothing proved yet");
    seed_kitsu(&state, "finished");
    check_availability(&state, &availability_args())
        .await
        .expect("answered");
    assert_eq!(read(&state), "12.1");
}

/// A row from before history recorded the Kitsu id is claimed by the
/// show id's mapping, on the detail page's lookup and in Continue
/// alike, and reads back the same way.
#[tokio::test]
async fn a_row_older_than_the_recorded_id_reads_back_as_12_1_through_its_mapping() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    let row = crate::history::HistoryEntry {
        ep_no: "13".into(),
        id: "hianime:noragami-916".into(),
        title: "Noragami".into(),
        watched_at: None,
        kitsu_id: None,
        own_number: false,
    };
    crate::history::write_atomic(&state.history_path, std::slice::from_ref(&row)).expect("history");
    crate::commands::kitsu::allmanga_kitsu_put(&state, &row.id, KITSU).expect("mapping");
    seed_kitsu(&state, "finished");
    check_availability(&state, &availability_args())
        .await
        .expect("answered");
    let listed = crate::commands::history::history_list(&state).expect("history");
    assert_eq!(listed[0].ep_no, "12.1");
    let found = crate::commands::history::history_by_kitsu(&state, KITSU)
        .expect("history")
        .expect("the row");
    assert_eq!(found.ep_no, "12.1");
}

/// Kitsu recounting the show — the OADs counted as episodes 13 and 14
/// — leaves no listing past the count, and the next resolve of the
/// same listing forgets the reading: row 13 is then episode 13.
#[tokio::test]
async fn a_recount_that_takes_the_rows_in_forgets_the_reading() {
    let td = tempfile::tempdir().expect("td");
    let provider = hianime();
    let state = state_over(&td, &provider);
    seed_kitsu(&state, "finished");
    check_availability(&state, &availability_args())
        .await
        .expect("answered");
    let recount: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Noragami",
        "mode": "sub",
        "episode_count": 14,
        "kitsu_id": KITSU,
        "status": "finished",
        "bypass_cache": true,
    }))
    .expect("args");
    let got = check_availability(&state, &recount)
        .await
        .expect("answered");
    assert_eq!(got.episode_count, Some(14));
    let row = crate::history::HistoryEntry {
        ep_no: "13".into(),
        id: "hianime:noragami-916".into(),
        title: "Noragami".into(),
        watched_at: None,
        kitsu_id: Some(KITSU.into()),
        own_number: false,
    };
    crate::history::write_atomic(&state.history_path, std::slice::from_ref(&row)).expect("history");
    assert_eq!(
        crate::commands::history::history_list(&state).expect("history")[0].ep_no,
        "13"
    );
}
