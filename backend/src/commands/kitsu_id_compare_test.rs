//! Every comparison between a stored Kitsu id and a requested one
//! reads both sides by the same rule (`crate::history::kitsu_id_in`).
//! A row written before the routes refused non-ids can hold a padded
//! id; the routes now hand the renderer its digits and pass the
//! digits back, so a comparison of the raw value would miss the row
//! it is about.

use std::collections::HashSet;
use std::sync::Arc;

use tempfile::TempDir;

use crate::app::AppState;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::commands::kitsu as k;

const PADDED: &str = " 49877 ";
const DIGITS: &str = "49877";

fn state(td: &TempDir) -> Arc<AppState> {
    Arc::new(AppState {
        anidb_base: None,
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: td.path().join("history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: td.path().join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
        ),
        config_path: td.path().join("config.toml"),
        state_dir: td.path().join("state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

/// The cross-cour guard hands the named eviction the stored value it
/// condemned, padded as stored; the eviction still removes it.
#[test]
fn a_named_eviction_of_the_stored_padded_value_removes_it() {
    let td = TempDir::new().expect("tempdir");
    let s = state(&td);
    k::allmanga_kitsu_put(&s, "show-a", PADDED).expect("seed");
    k::allmanga_kitsu_delete_named(&s, "show-a", PADDED).expect("evict");
    assert_eq!(k::allmanga_kitsu_get(&s, "show-a").expect("read"), None);
}

/// A different id still differs, however either side is padded.
#[test]
fn a_named_eviction_of_another_id_keeps_the_mapping() {
    let td = TempDir::new().expect("tempdir");
    let s = state(&td);
    k::allmanga_kitsu_put(&s, "show-a", PADDED).expect("seed");
    k::allmanga_kitsu_delete_named(&s, "show-a", " 1555 ").expect("evict");
    assert_eq!(
        k::allmanga_kitsu_get(&s, "show-a")
            .expect("read")
            .as_deref(),
        Some(PADDED)
    );
}

/// A mapping a play stored is the one its played mark names, padded
/// or not.
#[test]
fn a_played_mark_names_its_mapping_by_the_digits() {
    let td = TempDir::new().expect("tempdir");
    let s = state(&td);
    k::allmanga_kitsu_put(&s, "show-a", DIGITS).expect("seed");
    meta_cache_put(
        &s.cache_pool,
        &k::allmanga_kitsu_played_key("show-a"),
        PADDED,
        3600,
    )
    .expect("seed");
    assert!(crate::commands::kitsu_played::played_mapping(&s, "show-a")
        .expect("read")
        .is_some());
}

/// A row is named by its recorded id, its mapping or its title match
/// when the digits agree, whichever side carries the padding.
#[test]
fn a_row_is_named_by_an_id_whose_digits_agree() {
    use crate::commands::history_claim::{names, RowIds};
    let matched = [PADDED.to_owned()];
    let none: [String; 0] = [];
    let recorded = RowIds {
        recorded: Some((PADDED, false)),
        mapped: None,
        title_matched: &none,
    };
    let mapped = RowIds {
        recorded: None,
        mapped: Some((PADDED, false)),
        title_matched: &none,
    };
    let titled = RowIds {
        recorded: Some(("1", true)),
        mapped: None,
        title_matched: &matched,
    };
    for (what, row) in [
        ("recorded", recorded),
        ("mapped", mapped),
        ("titled", titled),
    ] {
        assert!(names(DIGITS, false, &row), "{what}, digits asked");
        assert!(names(PADDED, false, &row), "{what}, padded asked");
    }
    let other = RowIds {
        recorded: None,
        mapped: Some(("../49877", false)),
        title_matched: &none,
    };
    assert!(!names("../49877", false, &other), "a non-id names nothing");
}

fn seed_known_by(s: &AppState) {
    meta_cache_put(&s.cache_pool, "aniskip:v2:49877:1:1", "[]", 3600).expect("seed");
    meta_cache_put(&s.cache_pool, "kitsu:dead:49877", "1", 3600).expect("seed");
}

/// Removing a show forgets what is keyed by the id its padded mapping
/// names, under the digits those keys carry.
#[test]
fn a_removal_forgets_by_the_digits_of_a_padded_mapping() {
    let td = TempDir::new().expect("tempdir");
    let s = state(&td);
    k::allmanga_kitsu_put(&s, "show-a", PADDED).expect("seed");
    seed_known_by(&s);
    let forgot = crate::commands::history_forget::forget_show(
        &s,
        "show-a",
        "Show A",
        None,
        &[],
        &HashSet::new(),
    )
    .expect("forget");
    assert_eq!(forgot, vec![DIGITS.to_owned()]);
    assert_eq!(
        meta_cache_get(&s.cache_pool, "aniskip:v2:49877:1:1").expect("read"),
        None
    );
    assert_eq!(
        meta_cache_get(&s.cache_pool, "kitsu:dead:49877").expect("read"),
        None
    );
}

/// An id a remaining row claims stays with it, whichever side is
/// padded.
#[test]
fn a_claimed_id_is_kept_whichever_side_is_padded() {
    let td = TempDir::new().expect("tempdir");
    let s = state(&td);
    k::allmanga_kitsu_put(&s, "show-a", PADDED).expect("seed");
    seed_known_by(&s);
    let claimed: HashSet<String> = [DIGITS.to_owned()].into();
    let forgot =
        crate::commands::history_forget::forget_show(&s, "show-a", "Show A", None, &[], &claimed)
            .expect("forget");
    assert!(forgot.is_empty(), "{forgot:?}");
    assert!(meta_cache_get(&s.cache_pool, "aniskip:v2:49877:1:1")
        .expect("read")
        .is_some());
}

/// A title match under another title that names a forgotten id goes,
/// though it stored the id padded.
#[test]
fn a_title_match_naming_a_forgotten_id_goes_though_padded() {
    let td = TempDir::new().expect("tempdir");
    let s = state(&td);
    let provider = crate::scraper::provider::ProviderId::Anidb;
    k::title_match_put(&s, provider, "Elsewhere", 1, PADDED).expect("seed");
    crate::commands::history_forget_titles::forget_title_matches_naming(
        &s,
        "show-a",
        &["Show A"],
        &[DIGITS.to_owned()],
    )
    .expect("forget");
    assert_eq!(
        k::title_match_get(&s, provider, "Elsewhere", 1).expect("read"),
        None
    );
}

/// A resolution row resolved from a page the show was known by is
/// found, though the row stored the page padded.
#[test]
fn a_resolution_from_a_padded_page_is_found() {
    let td = TempDir::new().expect("tempdir");
    let s = state(&td);
    let row = serde_json::json!({ "show_id": "other-show", "kitsu_id": PADDED });
    meta_cache_put(&s.cache_pool, "play:v1:x", &row.to_string(), 3600).expect("seed");
    let found = crate::commands::history_forget_resolutions::find_resolutions(
        &s,
        &[],
        &[DIGITS.to_owned()],
    )
    .expect("find");
    assert_eq!(found.named, vec!["other-show".to_owned()]);
}
