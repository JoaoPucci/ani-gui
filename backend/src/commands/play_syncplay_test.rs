use super::*;
use crate::commands::play::tests::walked_key;
use crate::commands::play::PlayArgs;
use crate::scraper::table_provider::TableProvider;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The same one-show provider as the play_external tests: a
/// two-episode listing, the second streaming from a master on `mock`.
fn stub_provider(mock: &MockServer, query: &str) -> TableProvider {
    TableProvider::one_show(
        query,
        ("the-show-77", "The Show"),
        &[(701, 1), (702, 2)],
        &[702],
        &format!("{}/x/master.m3u8", mock.uri()),
    )
}

fn stage_recorder(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let argv_file = dir.join("syncplay-argv");
    let binary = dir.join("recorder");
    std::fs::write(
        &binary,
        format!(
            "#!/bin/sh\n{}\n",
            crate::stub_file::publish(&argv_file, "printf '%s\\n' \"$@\"")
        ),
    )
    .expect("write recorder");
    let mut perms = std::fs::metadata(&binary).expect("meta").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&binary, perms).expect("chmod");
    (binary, argv_file)
}

fn state_for(dir: &std::path::Path, provider: &Arc<TableProvider>) -> crate::app::AppState {
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
        history_path: dir.join("history"),
        providers: crate::provider_set::ProviderSet::for_tests().with_stub(Arc::clone(provider)),
        image_cache_dir: dir.join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::new(reqwest::Client::new()),
        config_path: dir.join("config.toml"),
        state_dir: dir.join("state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        flags: crate::flags::FlagStore::fixed(crate::flags::DECLARED),
    }
}

fn play_args() -> PlayArgs {
    PlayArgs {
        locale: None,
        title: "the show".into(),
        episode: "2".into(),
        mode: "sub".into(),
        subtype: None,
        episode_count: Some(2),
        year: None,
        alt_titles: Vec::new(),
        prefetch: false,
        record_later: false,
        kitsu_id: None,
        version: None,
        guess_row: None,
        card_row: None,
        guess_id: None,
        caption: None,
    }
}

#[tokio::test]
async fn a_fresh_syncplay_launch_resolves_natively_and_receives_the_master_url() {
    // Empty cache forces the fresh path; the provider is unreachable,
    // so success proves the resolution never left the native client.
    let mock = MockServer::start().await;
    let provider = Arc::new(stub_provider(&mock, "the show"));
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, argv_file) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &provider);
    std::fs::write(
        &state.config_path,
        format!(
            "syncplay_binary = {}\n",
            crate::stub_file::toml_quote(&binary)
        ),
    )
    .expect("write config");

    play_syncplay(&state, &play_args()).await.expect("launches");

    let mut argv = String::new();
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(&argv_file) {
            if !s.is_empty() {
                argv = s;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        argv.contains(&format!("{}/x/master.m3u8", mock.uri())),
        "syncplay must receive the master playlist URL; got: {argv}"
    );
}

/// Syncplay's cache-hit launch records the watch once the spawn
/// succeeded, exactly as the external player's does.
#[tokio::test]
async fn a_cached_syncplay_launch_records_the_watch_after_the_spawn() {
    let mock = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/cached/master.m3u8"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, argv_file) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &Arc::new(TableProvider::unreachable()));
    std::fs::write(
        &state.config_path,
        format!(
            "syncplay_binary = {}\ncache_resolutions = true\n",
            crate::stub_file::toml_quote(&binary)
        ),
    )
    .expect("write config");
    let args = play_args();
    let key = crate::commands::play_resolution_cache::cache_key(
        &args.title,
        &args.mode,
        &args.episode,
        args.year,
        args.episode_count,
        args.subtype.as_deref(),
    );
    crate::commands::play_resolution_cache::put(
        &state.cache_pool,
        &key,
        &crate::commands::play_resolution_cache::CachedResolution {
            upstream_url: format!("{}/cached/master.m3u8", mock.uri()),
            referer: String::new(),
            media_kind: crate::proxy::MediaKind::Hls,
            show_id: walked_key("cached-show-9"),
            show_title: "Cached Show".into(),
            resolved_slot: Some(2),
            subtitles: Vec::new(),
            kitsu_id: None,
            versions: Vec::new(),
            version: None,
            played: None,
            former_episode: None,
            episode_unverified: false,
        },
    );

    play_syncplay(&state, &args)
        .await
        .expect("launches from the cache");

    let mut argv = String::new();
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(&argv_file) {
            if !s.is_empty() {
                argv = s;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        argv.contains("/cached/master.m3u8"),
        "the cached URL reached Syncplay: {argv}"
    );
    let hsts = std::fs::read_to_string(&state.history_path).expect("history written");
    assert!(hsts.contains("cached-show-9"), "{hsts}");
    assert!(
        crate::commands::kitsu::watched_at_get(&state, &walked_key("cached-show-9"))
            .expect("stamp read")
            .is_some(),
        "the watch is stamped once Syncplay started"
    );
}

/// Syncplay's handoff records its watch the way the external player's
/// does, the show's reverse mapping included, so the row it leaves is
/// found by the Kitsu id and weighed by its stamp.
#[tokio::test]
async fn a_cached_syncplay_launch_persists_the_shows_kitsu_mapping() {
    let mock = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/cached/master.m3u8"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, argv_file) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &Arc::new(TableProvider::unreachable()));
    std::fs::write(
        &state.config_path,
        format!(
            "syncplay_binary = {}\ncache_resolutions = true\n",
            crate::stub_file::toml_quote(&binary)
        ),
    )
    .expect("write config");
    let args = PlayArgs {
        kitsu_id: Some("42".into()),
        ..play_args()
    };
    let key = crate::commands::play_resolution_cache::cache_key(
        &args.title,
        &args.mode,
        &args.episode,
        args.year,
        args.episode_count,
        args.subtype.as_deref(),
    );
    crate::commands::play_resolution_cache::put(
        &state.cache_pool,
        &key,
        &crate::commands::play_resolution_cache::CachedResolution {
            upstream_url: format!("{}/cached/master.m3u8", mock.uri()),
            referer: String::new(),
            media_kind: crate::proxy::MediaKind::Hls,
            show_id: walked_key("cached-show-9"),
            show_title: "Cached Show".into(),
            resolved_slot: Some(2),
            subtitles: Vec::new(),
            kitsu_id: None,
            versions: Vec::new(),
            version: None,
            played: None,
            former_episode: None,
            episode_unverified: false,
        },
    );

    play_syncplay(&state, &args)
        .await
        .expect("launches from the cache");
    for _ in 0..100 {
        if std::fs::read_to_string(&argv_file).is_ok_and(|s| !s.is_empty()) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    assert_eq!(
        crate::commands::kitsu::allmanga_kitsu_get(&state, &walked_key("cached-show-9"))
            .expect("mapping read"),
        Some("42".into()),
        "the spawn persists the show's reverse mapping"
    );
}

/// Syncplay's handoff resolves before it launches, like the external
/// player's: a show removed from history while it resolves is not
/// recorded by the launch that follows.
#[tokio::test]
async fn a_syncplay_launch_begun_before_the_show_was_removed_records_nothing() {
    let mock = MockServer::start().await;
    // Hold the resolve on its first request.
    let (hold, gate) = crate::held::gate();
    let provider = Arc::new(stub_provider(&mock, "the show").holding_searches(hold));
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, argv_file) = stage_recorder(dir.path());
    let state = std::sync::Arc::new(state_for(dir.path(), &provider));
    std::fs::write(
        &state.config_path,
        format!(
            "syncplay_binary = {}\n",
            crate::stub_file::toml_quote(&binary)
        ),
    )
    .expect("write config");
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: walked_key("the-show-77"),
            title: "The Show".into(),
            watched_at: None,
            kitsu_id: None,
            own_number: false,
        },
    )
    .expect("seed row");

    let mut launch = {
        let state = std::sync::Arc::clone(&state);
        tokio::spawn(async move { play_syncplay(&state, &play_args()).await })
    };
    tokio::select! {
        () = gate.asked() => {}
        ended = &mut launch => panic!("the handoff ended before it was held: {ended:?}"),
    }
    assert!(
        crate::commands::history::history_delete(&state, &walked_key("the-show-77"))
            .expect("delete")
    );
    gate.release();
    launch.await.expect("join").expect("launches");
    let mut argv = String::new();
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(&argv_file) {
            if !s.is_empty() {
                argv = s;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    assert!(
        argv.contains("/x/master.m3u8"),
        "syncplay was not launched with the stream: {argv:?}"
    );
    assert!(
        crate::history::read_all(&state.history_path)
            .expect("rows")
            .is_empty(),
        "the removed show has no row"
    );
    assert_eq!(
        crate::commands::kitsu::watched_at_get(&state, &walked_key("the-show-77")).expect("stamp"),
        None
    );
    let offsets = std::fs::read_to_string(dir.path().join("ani-gui-offsets")).unwrap_or_default();
    assert!(!offsets.contains("the-show-77"), "nor numbering: {offsets}");
}

/// Syncplay's handoff checks a cached stream before it resolves afresh,
/// like the external player's, and takes one moment before that check:
/// a show removed while the check waits on the CDN gets no numbering
/// from the fresh resolve that follows a dead cached stream.
#[tokio::test]
async fn a_syncplay_launch_whose_show_was_removed_during_the_cache_check_stamps_no_numbering() {
    let mock = MockServer::start().await;
    // The cached stream is dead, and says so once the removal is in.
    let (hold, gate) = crate::held::gate();
    Mock::given(method("HEAD"))
        .and(path("/cached/master.m3u8"))
        .respond_with(crate::held::Held::new(hold, ResponseTemplate::new(404)))
        .mount(&mock)
        .await;
    let provider = Arc::new(stub_provider(&mock, "the show"));
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, argv_file) = stage_recorder(dir.path());
    let state = std::sync::Arc::new(state_for(dir.path(), &provider));
    std::fs::write(
        &state.config_path,
        format!(
            "syncplay_binary = {}\ncache_resolutions = true\n",
            crate::stub_file::toml_quote(&binary)
        ),
    )
    .expect("write config");
    let args = play_args();
    let key = crate::commands::play_resolution_cache::cache_key(
        &args.title,
        &args.mode,
        &args.episode,
        args.year,
        args.episode_count,
        args.subtype.as_deref(),
    );
    crate::commands::play_resolution_cache::put(
        &state.cache_pool,
        &key,
        &crate::commands::play_resolution_cache::CachedResolution {
            upstream_url: format!("{}/cached/master.m3u8", mock.uri()),
            referer: String::new(),
            media_kind: crate::proxy::MediaKind::Hls,
            show_id: walked_key("the-show-77"),
            show_title: "The Show".into(),
            resolved_slot: Some(2),
            subtitles: Vec::new(),
            kitsu_id: None,
            versions: Vec::new(),
            version: None,
            played: None,
            former_episode: None,
            episode_unverified: false,
        },
    );
    crate::history::upsert_and_write(
        &state.history_path,
        crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: walked_key("the-show-77"),
            title: "The Show".into(),
            watched_at: None,
            kitsu_id: None,
            own_number: false,
        },
    )
    .expect("seed row");

    let mut launch = {
        let state = std::sync::Arc::clone(&state);
        tokio::spawn(async move { play_syncplay(&state, &play_args()).await })
    };
    tokio::select! {
        () = gate.asked() => {}
        ended = &mut launch => panic!("the handoff ended before it was held: {ended:?}"),
    }
    assert!(
        crate::commands::history::history_delete(&state, &walked_key("the-show-77"))
            .expect("delete")
    );
    gate.release();
    launch.await.expect("join").expect("launches");
    let mut argv = String::new();
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(&argv_file) {
            if !s.is_empty() {
                argv = s;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    assert!(
        argv.contains("/x/master.m3u8"),
        "the fresh stream reached syncplay: {argv}"
    );
    let offsets = std::fs::read_to_string(dir.path().join("ani-gui-offsets")).unwrap_or_default();
    assert!(!offsets.contains("the-show-77"), "no numbering: {offsets}");
    assert!(
        crate::history::read_all(&state.history_path)
            .expect("rows")
            .is_empty(),
        "and no row"
    );
}

/// The guessed card the launches below are asked from: a line of
/// another listing than the one its guess plays.
fn guessed_card() -> String {
    walked_key("the-show-2000-5")
}

/// `args` as a guessed Continue card's player sends them: the card's
/// row, and its guess named apart from the page's id.
fn guessed_args(row: &str, guess: &str) -> PlayArgs {
    PlayArgs {
        guess_row: Some(row.into()),
        card_row: None,
        guess_id: Some(guess.into()),
        ..play_args()
    }
}

/// Syncplay configured as the recorder, with the play cache on when
/// `cached`.
fn configure_syncplay(state: &crate::app::AppState, binary: &std::path::Path, cached: bool) {
    std::fs::write(
        &state.config_path,
        format!(
            "syncplay_binary = {}\ncache_resolutions = {cached}\n",
            crate::stub_file::toml_quote(binary)
        ),
    )
    .expect("write config");
}

/// The Kitsu id the history row `row` records.
fn recorded_id(state: &crate::app::AppState, row: &str) -> Option<String> {
    crate::history::read_all(&state.history_path)
        .expect("rows")
        .into_iter()
        .find(|r| r.id == row)
        .expect("the row is written")
        .kitsu_id
}

/// A guessed card's Syncplay launch that landed on another listing
/// notes, beside the row it wrote, the card it came from, as the same
/// play in the player does: "Not this show?" on Home then corrects the
/// line the user clicked, not the listing the launch wrote.
#[tokio::test]
async fn a_guessed_cards_syncplay_launch_of_another_listing_notes_the_card() {
    let mock = MockServer::start().await;
    let provider = Arc::new(stub_provider(&mock, "the show"));
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, _argv) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &provider);
    configure_syncplay(&state, &binary, false);

    play_syncplay(&state, &guessed_args(&guessed_card(), "42"))
        .await
        .expect("launches");

    let played = walked_key("the-show-77");
    assert_eq!(
        crate::commands::guessed_from::all(&state)
            .expect("notes")
            .get(&played),
        Some(&guessed_card()),
    );
    assert_eq!(recorded_id(&state, &played), None);
}

#[tokio::test]
async fn a_guessed_cards_cached_syncplay_launch_of_another_listing_notes_the_card() {
    let mock = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/cached/master.m3u8"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, _argv) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &Arc::new(TableProvider::unreachable()));
    configure_syncplay(&state, &binary, true);
    let args = guessed_args(&guessed_card(), "42");
    let played = walked_key("cached-show-9");
    let key = crate::commands::play_resolution_cache::cache_key(
        &args.title,
        &args.mode,
        &args.episode,
        args.year,
        args.episode_count,
        args.subtype.as_deref(),
    );
    crate::commands::play_resolution_cache::put(
        &state.cache_pool,
        &key,
        &crate::commands::play_resolution_cache::CachedResolution {
            upstream_url: format!("{}/cached/master.m3u8", mock.uri()),
            referer: String::new(),
            media_kind: crate::proxy::MediaKind::Hls,
            show_id: played.clone(),
            show_title: "Cached Show".into(),
            resolved_slot: Some(2),
            subtitles: Vec::new(),
            kitsu_id: None,
            versions: Vec::new(),
            version: None,
            played: None,
            former_episode: None,
            episode_unverified: false,
        },
    );

    play_syncplay(&state, &args)
        .await
        .expect("launches from the cache");

    assert_eq!(
        crate::commands::guessed_from::all(&state)
            .expect("notes")
            .get(&played),
        Some(&guessed_card()),
    );
}

/// A guessed card's launch that landed on the card's own listing
/// records the card's guess on its row, as the player's play does.
#[tokio::test]
async fn a_guessed_cards_syncplay_launch_of_its_own_listing_records_the_guess() {
    let mock = MockServer::start().await;
    let provider = Arc::new(stub_provider(&mock, "the show"));
    let dir = tempfile::tempdir().expect("tmp");
    let (binary, _argv) = stage_recorder(dir.path());
    let state = state_for(dir.path(), &provider);
    configure_syncplay(&state, &binary, false);
    let row = walked_key("the-show-77");

    play_syncplay(&state, &guessed_args(&row, "42"))
        .await
        .expect("launches");

    assert_eq!(recorded_id(&state, &row).as_deref(), Some("42"));
}
