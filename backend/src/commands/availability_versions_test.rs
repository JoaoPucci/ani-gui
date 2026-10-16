//! The availability answer names a show's versions, and its row keeps
//! them for every later reader.

use super::tests::cache_only_state;
use super::*;
use crate::commands::play_native_versions::{VersionInfo, VersionKind};
use crate::scraper::provider::{BrowseHit, ShowKey};
use crate::scraper::table_provider::{listing_of, TableProvider};
use std::sync::Arc;

const YEAR: u32 = 2020;

fn key(slug: &str) -> String {
    ShowKey::new(crate::provider_set::ProviderSet::TEST_DEFAULT, slug).to_string()
}

fn card(slug: &str, title: &str) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
        jname: None,
    }
}

/// "The Show" listed twice, three episodes each, every row carrying sub.
fn two_copies() -> Arc<TableProvider> {
    let table = TableProvider::new()
        .search_default(vec![
            card("the-show-1", "The Show"),
            card("the-show-2", "The Show (Uncensored)"),
        ])
        .listing(1, listing_of(1, 3))
        .listing(2, listing_of(2, 3))
        .year(1, Some(YEAR))
        .year(2, Some(YEAR));
    Arc::new(
        [1001, 1002, 1003, 2001, 2002, 2003]
            .into_iter()
            .fold(table, |t, row| t.mode(row, "sub", true)),
    )
}

fn args() -> AvailabilityArgs {
    AvailabilityArgs {
        title: "The Show".into(),
        mode: "sub".into(),
        alt_titles: Vec::new(),
        episode_count: Some(3),
        year: Some(YEAR),
        subtype: Some("TV".into()),
        kitsu_id: Some("777".into()),
        status: Some("finished".into()),
        background: false,
        bypass_cache: false,
    }
}

fn both_versions() -> Vec<VersionInfo> {
    vec![
        VersionInfo {
            kind: VersionKind::Standard,
            show_key: key("the-show-1"),
            has_mode: None,
        },
        VersionInfo {
            kind: VersionKind::Uncensored,
            show_key: key("the-show-2"),
            has_mode: None,
        },
    ]
}

#[tokio::test]
async fn the_answer_names_the_versions_and_the_row_keeps_them() {
    let td = tempfile::tempdir().expect("tempdir");
    let mut state = cache_only_state(&td);
    let provider = two_copies();
    state.providers.set_stub(Arc::clone(&provider));
    let got = check_availability(&state, &args()).await.expect("answered");
    assert!(got.available);
    assert_eq!(got.versions, both_versions());
    // A later look is served from the row, versions and all.
    let asked = provider.asked().len();
    let again = check_availability(&state, &args()).await.expect("answered");
    assert_eq!(provider.asked().len(), asked, "served from the row");
    assert_eq!(again.versions, both_versions());
}

#[tokio::test]
async fn a_served_resolve_stamps_its_versions_into_the_row() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    let at_start = RowAtStart::read(&state, Some("777"), "sub").await;
    let versions = both_versions();
    stamp_after_native(
        &state,
        Some("777"),
        "sub",
        &at_start,
        ResolveVerdict::served(crate::provider_set::ProviderSet::TEST_DEFAULT, Some(3), &[])
            .with_versions(&versions),
    )
    .await;
    let body = crate::cache::meta_cache_get(&state.cache_pool, &cache_key("777", "sub"))
        .expect("read")
        .expect("a row");
    let row: AvailabilityResponse = serde_json::from_str(&body).expect("row");
    assert_eq!(row.versions, both_versions());
}

/// A row probed before picks had versions cannot say whether its show
/// has more than one, and would hide the choice for its whole lifetime.
#[test]
fn a_row_probed_before_versions_is_not_read() {
    let current = cache_key("7158", "sub");
    assert!(
        !current.starts_with("availability:v19:"),
        "the key must have moved past v19: {current}"
    );
}
