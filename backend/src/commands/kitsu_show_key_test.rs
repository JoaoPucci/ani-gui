//! The reverse resolver and the mapping guard over qualified ids.

use super::*;
use crate::app::AppState;
use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
use std::path::PathBuf;
use std::sync::Arc;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SEARCH_FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/kitsu/search_one_piece.json");
const DETAIL_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");

fn state_with_kitsu_at(uri: &str) -> AppState {
    AppState {
        anidb_base: None,
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 0),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui-history-never-read"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// A row keyed under another provider's label resolves through its
/// slug's words like an anidb row, and persists under the id asked.
#[tokio::test]
async fn a_qualified_id_resolves_through_its_slugs_words() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param("filter[text]", "one piece"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(SEARCH_FIXTURE.to_vec()),
        )
        .mount(&mock)
        .await;
    let state = state_with_kitsu_at(&mock.uri());
    let got = resolve_allmanga_show_id(&state, "hianime:one-piece-100", false)
        .await
        .expect("resolve ok");
    assert_eq!(got.expect("qualified rows resolve").id, "12");
    assert_eq!(
        allmanga_kitsu_get(&state, "hianime:one-piece-100")
            .expect("cache read")
            .as_deref(),
        Some("12"),
        "the mapping persists under the qualified id"
    );
}

/// The cross-cour guard compares the provider's title against Kitsu's
/// slug convention, and it guards every provider's ids: the resolve
/// carries no identity the guard could defer to — no Kitsu or
/// MyAnimeList id, only the title, year and count the picker
/// scored — and a wrongly picked sibling cour would otherwise
/// persist the caller's Kitsu id under its slug, which is the poison
/// Continue Watching and resume then read.
#[tokio::test]
async fn the_cour_guard_applies_to_every_providers_ids() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(DETAIL_FIXTURE.to_vec()),
        )
        .mount(&mock)
        .await;
    let state = state_with_kitsu_at(&mock.uri());
    // Kitsu's One Piece slug carries no part suffix (cour 1); the
    // provider title says Part 2. For anidb that is the poison the
    // guard exists for.
    try_put_allmanga_kitsu_mapping(&state, "one-piece-69", "One Piece Part 2", "12").await;
    assert_eq!(
        allmanga_kitsu_get(&state, "one-piece-69").expect("read"),
        None,
        "an anidb id under a cross-cour title stays unmapped"
    );
    try_put_allmanga_kitsu_mapping(&state, "hianime:one-piece-100", "One Piece Part 2", "12").await;
    assert_eq!(
        allmanga_kitsu_get(&state, "hianime:one-piece-100").expect("read"),
        None,
        "another provider's id under a cross-cour title stays unmapped too"
    );
}
