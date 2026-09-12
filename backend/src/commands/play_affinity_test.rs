//! The play walk against a positive availability row: what a miss
//! reached past the row's provider may and may not do to the row.

use super::*;
use crate::commands::availability::{cached_provider, write_cache};
use crate::scraper::provider::ProviderId;

/// An `AppState` over an in-memory cache with both providers pointed
/// at the given mocks. Mirrors the play tests' own builder, which is
/// private to their module.
fn state_over(anidb: &wiremock::MockServer, hianime: &wiremock::MockServer) -> AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::sync::Arc;
    AppState {
        anidb_base: Some(anidb.uri()),
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: std::path::PathBuf::from("/tmp/ani-gui/history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: Some(hianime.uri()),
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![ProviderId::Anidb, ProviderId::Hianime],
        image_cache_dir: std::path::PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::new(reqwest::Client::new()),
        config_path: std::path::PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: std::path::PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// A hianime that lists the show with one episode, so a play of its
/// second episode is an episode dead end: the show found, the
/// episode not served.
async fn hianime_with_one_episode() -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    let server = wiremock::MockServer::start().await;
    let base = server.uri();
    let search = format!(
        r#"<div class="film_list-wrap"><div class="flw-item"><div class="film-detail"><h3 class="film-name"><a href="{base}/the-show-7" title="The Show" class="dynamic-name">The Show</a></h3><div class="fd-infor"><span class="fdi-item">TV</span></div></div></div></div><div id="main-sidebar"></div>"#
    );
    wiremock::Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(search))
        .mount(&server)
        .await;
    let list = serde_json::json!({
        "status": true,
        "html": r#"<a class="ep-item" data-number="1" data-id="7001"></a>"#,
    })
    .to_string();
    wiremock::Mock::given(method("GET"))
        .and(path("/api/theme/episode/list/7"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(list))
        .mount(&server)
        .await;
    server
}

/// The positive row remembers hianime, which listed the show; a play
/// of an episode hianime does not serve moves on to anidb.app, which
/// has never heard of the show. That clean miss used to be the walk's
/// verdict, and the play persisted it: a negative row named after a
/// healthy primary, backed for its whole lifetime, hiding a title
/// hianime plays but for one episode. The remembered provider found
/// the show, so its own dead end is the verdict and the row stands.
#[tokio::test]
async fn an_episode_hianime_does_not_serve_does_not_write_anidbs_catalogue_miss_over_its_row() {
    let anidb = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"<div class="grid"><p>No results.</p></div>"#),
        )
        .mount(&anidb)
        .await;
    let hianime = hianime_with_one_episode().await;
    let state = state_over(&anidb, &hianime);
    write_cache(&state, "580", "sub", true, Some(ProviderId::Hianime));
    let args = PlayArgs {
        title: "The Show".into(),
        episode: "2".into(),
        mode: "sub".into(),
        quality: None,
        subtype: None,
        episode_count: None,
        year: None,
        alt_titles: vec![],
        prefetch: false,
        kitsu_id: Some("580".into()),
    };
    let got = play_with_progress(&state, &args, |_| {}).await;
    assert!(
        matches!(got, Err(crate::error::AniError::NoResults)),
        "the episode is nowhere: {got:?}"
    );
    assert!(
        hianime
            .received_requests()
            .await
            .expect("recorded")
            .iter()
            .any(|r| r.url.path().ends_with("/episode/list/7")),
        "hianime found the show and listed its episodes"
    );
    assert!(
        !anidb
            .received_requests()
            .await
            .expect("recorded")
            .is_empty(),
        "the walk moved on to anidb.app after the dead end"
    );
    assert_eq!(
        cached_provider(&state, "580", "sub"),
        Some(ProviderId::Hianime),
        "the positive row stands: hianime found the show"
    );
}
