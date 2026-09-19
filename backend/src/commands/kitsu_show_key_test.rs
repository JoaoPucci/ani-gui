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
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
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

/// The One Piece detail fixture re-keyed to another entry: a sibling
/// cour, its id and slug the only fields that differ.
fn sibling_cour_detail(id: &str, slug: &str) -> Vec<u8> {
    let mut detail: serde_json::Value = serde_json::from_slice(DETAIL_FIXTURE).expect("fixture");
    detail["data"]["id"] = serde_json::Value::from(id);
    detail["data"]["attributes"]["slug"] = serde_json::Value::from(slug);
    serde_json::to_vec(&detail).expect("json")
}

async fn serve_detail(mock: &MockServer, id: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(format!("/anime/{id}")))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(body),
        )
        .mount(mock)
        .await;
}

/// A rejected write does not leave the show's old mapping standing
/// when the same evidence condemns it. A row stamped before the guard
/// existed can bind the show key to the very entry the guard now
/// refuses; the watch being recorded has just advanced that key's
/// stamp, and under the stale mapping `history_by_kitsu` would resume
/// the wrong entry from it over a row that maps correctly.
#[tokio::test]
async fn a_rejected_write_drops_the_old_mapping_under_the_refused_entry() {
    let mock = MockServer::start().await;
    serve_detail(&mock, "12", DETAIL_FIXTURE.to_vec()).await;
    let state = state_with_kitsu_at(&mock.uri());
    // The poison the guard was written against, persisted before it
    // existed: a Part 2 title bound to the cour-1 entry.
    allmanga_kitsu_put(&state, "one-piece-69", "12").expect("seed");
    try_put_allmanga_kitsu_mapping(&state, "one-piece-69", "One Piece Part 2", "12").await;
    assert_eq!(
        allmanga_kitsu_get(&state, "one-piece-69").expect("read"),
        None,
        "the mapping the guard just disproved is gone"
    );
}

/// The old mapping need not point at the refused entry to be wrong:
/// the title's cour evidence condemns any entry whose slug disagrees
/// with it. Both entries here are sibling cours of a Part 3 title,
/// and neither survives.
#[tokio::test]
async fn a_rejected_write_drops_an_old_mapping_the_title_disagrees_with() {
    let mock = MockServer::start().await;
    serve_detail(&mock, "12", DETAIL_FIXTURE.to_vec()).await;
    serve_detail(&mock, "13", sibling_cour_detail("13", "one-piece-part-2")).await;
    let state = state_with_kitsu_at(&mock.uri());
    allmanga_kitsu_put(&state, "one-piece-69", "12").expect("seed");
    try_put_allmanga_kitsu_mapping(&state, "one-piece-69", "One Piece Part 3", "13").await;
    assert_eq!(
        allmanga_kitsu_get(&state, "one-piece-69").expect("read"),
        None,
        "neither the refused entry nor the old one maps the key"
    );
}

/// The refusal condemns only what the evidence reaches. A stored
/// mapping whose entry agrees with the title's cour is the correct
/// one, and a play aimed at the wrong sibling must not cost it.
#[tokio::test]
async fn a_rejected_write_keeps_a_stored_mapping_the_title_agrees_with() {
    let mock = MockServer::start().await;
    serve_detail(&mock, "12", DETAIL_FIXTURE.to_vec()).await;
    serve_detail(&mock, "13", sibling_cour_detail("13", "one-piece-part-2")).await;
    let state = state_with_kitsu_at(&mock.uri());
    allmanga_kitsu_put(&state, "one-piece-69", "13").expect("seed");
    try_put_allmanga_kitsu_mapping(&state, "one-piece-69", "One Piece Part 2", "12").await;
    assert_eq!(
        allmanga_kitsu_get(&state, "one-piece-69")
            .expect("read")
            .as_deref(),
        Some("13"),
        "the correct mapping outlives a play aimed at the wrong cour"
    );
}

/// Silence is not disagreement: a stored entry Kitsu does not answer
/// for is neither proven nor disproven, and stays.
#[tokio::test]
async fn a_rejected_write_keeps_a_stored_mapping_it_cannot_check() {
    let mock = MockServer::start().await;
    serve_detail(&mock, "12", DETAIL_FIXTURE.to_vec()).await;
    let state = state_with_kitsu_at(&mock.uri());
    allmanga_kitsu_put(&state, "one-piece-69", "404").expect("seed");
    try_put_allmanga_kitsu_mapping(&state, "one-piece-69", "One Piece Part 2", "12").await;
    assert_eq!(
        allmanga_kitsu_get(&state, "one-piece-69")
            .expect("read")
            .as_deref(),
        Some("404"),
        "an entry that cannot be fetched is not condemned"
    );
}

// ── the enrichment path is guarded by cour too ────────────────────

/// A Kitsu search answer of the given entries, each the One Piece
/// record re-keyed: its id and slug are what tell sibling cours
/// apart.
fn search_body(entries: &[(&str, &str)]) -> Vec<u8> {
    let fixture: serde_json::Value = serde_json::from_slice(SEARCH_FIXTURE).expect("fixture");
    let template = fixture["data"][0].clone();
    let data: Vec<serde_json::Value> = entries
        .iter()
        .map(|(id, slug)| {
            let mut entry = template.clone();
            entry["id"] = serde_json::Value::from(*id);
            entry["attributes"]["slug"] = serde_json::Value::from(*slug);
            entry
        })
        .collect();
    serde_json::to_vec(&serde_json::json!({ "data": data })).expect("json")
}

async fn serve_search(mock: &MockServer, term: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param("filter[text]", term))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_bytes(body),
        )
        .mount(mock)
        .await;
}

/// The slug's words carry the show's cour like a title does, and the
/// enrichment resolve reads them: Kitsu ranking the parent cour first
/// for a Part 2 slug is the poison the mapping guard refuses on the
/// play path, so the resolve passes over it to the entry whose slug
/// agrees, and that is what it answers and persists.
#[tokio::test]
async fn the_enrichment_resolve_passes_over_a_hit_whose_cour_disagrees_with_the_slug() {
    let mock = MockServer::start().await;
    serve_search(
        &mock,
        "one piece part 2",
        search_body(&[("12", "one-piece"), ("13", "one-piece-part-2")]),
    )
    .await;
    let state = state_with_kitsu_at(&mock.uri());
    let got = resolve_allmanga_show_id(&state, "hianime:one-piece-part-2-100", true)
        .await
        .expect("resolve ok");
    assert_eq!(
        got.expect("the agreeing entry resolves").id,
        "13",
        "the entry whose slug carries the same cour"
    );
    assert_eq!(
        allmanga_kitsu_get(&state, "hianime:one-piece-part-2-100")
            .expect("cache read")
            .as_deref(),
        Some("13"),
        "the agreeing entry is what persists"
    );
}

/// With no agreeing entry among the hits the resolve answers none
/// and persists nothing, rather than binding the Part 2 slug to the
/// parent cour: Continue Watching renders the bare title, and no row
/// stamped under the wrong entry can resume it.
#[tokio::test]
async fn the_enrichment_resolve_answers_none_when_every_hit_disagrees_with_the_slug() {
    let mock = MockServer::start().await;
    serve_search(
        &mock,
        "one piece part 2",
        search_body(&[("12", "one-piece")]),
    )
    .await;
    let state = state_with_kitsu_at(&mock.uri());
    let got = resolve_allmanga_show_id(&state, "hianime:one-piece-part-2-100", true)
        .await
        .expect("resolve ok");
    assert!(
        got.is_none(),
        "a disagreeing hit is not the mapping: {got:?}"
    );
    assert_eq!(
        allmanga_kitsu_get(&state, "hianime:one-piece-part-2-100").expect("cache read"),
        None,
        "nothing persists under the slug"
    );
}

/// Kitsu spells the same cour more than one way, and its slug for a
/// second season is as often `2nd-season` as `season-2`; the words
/// of a `season-2` slug agree with that hit, so it is the one
/// answered and persisted, ahead of the parent ranked first.
#[tokio::test]
async fn the_enrichment_resolve_reads_the_cour_of_an_ordinal_kitsu_slug() {
    let mock = MockServer::start().await;
    serve_search(
        &mock,
        "one piece season 2",
        search_body(&[("12", "one-piece"), ("13", "one-piece-2nd-season")]),
    )
    .await;
    let state = state_with_kitsu_at(&mock.uri());
    let got = resolve_allmanga_show_id(&state, "hianime:one-piece-season-2-100", true)
        .await
        .expect("resolve ok");
    assert_eq!(
        got.expect("the agreeing entry resolves").id,
        "13",
        "the entry whose slug spells the same cour as an ordinal"
    );
    assert_eq!(
        allmanga_kitsu_get(&state, "hianime:one-piece-season-2-100")
            .expect("cache read")
            .as_deref(),
        Some("13")
    );
}

/// The slug's own words can be the ordinal form too: a `2nd-season`
/// slug carries cour 2 like `season-2` does, and the parent hit is
/// passed over for it.
#[tokio::test]
async fn the_enrichment_resolve_reads_the_cour_of_an_ordinal_slug() {
    let mock = MockServer::start().await;
    serve_search(
        &mock,
        "one piece 2nd season",
        search_body(&[("12", "one-piece"), ("13", "one-piece-season-2")]),
    )
    .await;
    let state = state_with_kitsu_at(&mock.uri());
    let got = resolve_allmanga_show_id(&state, "hianime:one-piece-2nd-season-100", true)
        .await
        .expect("resolve ok");
    assert_eq!(got.expect("the agreeing entry resolves").id, "13");
}

/// The mapping guard reads the provider's title the same way: a
/// title ending in "2nd Season" names cour 2, and the parent's Kitsu
/// entry under it is the cross-cour pairing the guard refuses.
#[tokio::test]
async fn the_cour_guard_reads_an_ordinal_provider_title() {
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
    try_put_allmanga_kitsu_mapping(
        &state,
        "hianime:one-piece-100",
        "One Piece 2nd Season",
        "12",
    )
    .await;
    assert_eq!(
        allmanga_kitsu_get(&state, "hianime:one-piece-100").expect("read"),
        None,
        "an ordinal second season under the parent's entry stays unmapped"
    );
}

/// A slug without cour evidence has nothing to disagree with, so the
/// first entry answers as it always has, whichever provider's the
/// slug is.
#[tokio::test]
async fn the_enrichment_resolve_takes_the_first_hit_for_a_slug_without_cour_evidence() {
    let mock = MockServer::start().await;
    serve_search(
        &mock,
        "one piece",
        search_body(&[("12", "one-piece"), ("13", "one-piece-part-2")]),
    )
    .await;
    let state = state_with_kitsu_at(&mock.uri());
    let got = resolve_allmanga_show_id(&state, "one-piece-69", true)
        .await
        .expect("resolve ok");
    assert_eq!(got.expect("resolves").id, "12");
    assert_eq!(
        allmanga_kitsu_get(&state, "one-piece-69")
            .expect("cache read")
            .as_deref(),
        Some("12")
    );
}
