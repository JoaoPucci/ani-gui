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
    // Both copies carry sub, and the answer says so of each.
    let carrying: Vec<VersionInfo> = both_versions()
        .into_iter()
        .map(|v| VersionInfo {
            has_mode: Some(true),
            ..v
        })
        .collect();
    assert_eq!(got.versions, carrying);
    // A later look is served from the row, versions and all.
    let asked = provider.asked().len();
    let again = check_availability(&state, &args()).await.expect("answered");
    assert_eq!(provider.asked().len(), asked, "served from the row");
    assert_eq!(again.versions, carrying);
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

/// [`two_copies`], dubbed only in the plain copy.
fn dubbed_plain_copy() -> Arc<TableProvider> {
    let table = Arc::try_unwrap(two_copies()).unwrap_or_else(|_| unreachable!());
    Arc::new(
        [1001, 1002, 1003, 2001, 2002, 2003]
            .into_iter()
            .fold(table, |t, row| t.mode(row, "dub", row < 2000)),
    )
}

#[tokio::test]
async fn a_mode_only_one_version_carries_is_available_and_says_which() {
    let td = tempfile::tempdir().expect("tempdir");
    let mut state = cache_only_state(&td);
    state.providers.set_stub(dubbed_plain_copy());
    let dub = AvailabilityArgs {
        mode: "dub".into(),
        ..args()
    };
    let got = check_availability(&state, &dub).await.expect("answered");
    assert!(got.available, "the plain copy carries a dub");
    assert_eq!(got.episode_count, Some(3));
    let modes: Vec<(VersionKind, Option<bool>)> =
        got.versions.iter().map(|v| (v.kind, v.has_mode)).collect();
    assert_eq!(
        modes,
        vec![
            (VersionKind::Standard, Some(true)),
            (VersionKind::Uncensored, Some(false)),
        ]
    );
    // Sub, which both carry.
    let sub = check_availability(&state, &args()).await.expect("answered");
    let modes: Vec<Option<bool>> = sub.versions.iter().map(|v| v.has_mode).collect();
    assert_eq!(modes, vec![Some(true), Some(true)]);
}

#[tokio::test]
async fn a_mode_no_version_carries_is_absent() {
    let td = tempfile::tempdir().expect("tempdir");
    let mut state = cache_only_state(&td);
    let table = Arc::try_unwrap(two_copies()).unwrap_or_else(|_| unreachable!());
    state.providers.set_stub(Arc::new(
        [1001, 1002, 1003, 2001, 2002, 2003]
            .into_iter()
            .fold(table, |t, row| t.mode(row, "dub", false)),
    ));
    let dub = AvailabilityArgs {
        mode: "dub".into(),
        ..args()
    };
    let got = check_availability(&state, &dub).await.expect("answered");
    assert!(!got.available);
}

/// Master keys availability rows `availability:v20:` for the
/// appended-extras reading, whose picker names no versions, so this
/// key is past that one.
#[test]
fn a_row_keyed_v20_elsewhere_is_not_read() {
    let current = cache_key("7158", "sub");
    assert!(
        !current.starts_with("availability:v20:"),
        "the key must have moved past v20: {current}"
    );
}

/// [`two_copies`] with the plain copy's sub rows answering as
/// `plain`, the uncensored copy's as `tagged`.
fn modes_answering(
    plain: impl Fn() -> crate::scraper::table_provider::Answer<bool>,
    tagged: impl Fn() -> crate::scraper::table_provider::Answer<bool>,
) -> Arc<TableProvider> {
    let table = TableProvider::new()
        .search_default(vec![
            card("the-show-1", "The Show"),
            card("the-show-2", "The Show (Uncensored)"),
        ])
        .listing(1, listing_of(1, 3))
        .listing(2, listing_of(2, 3))
        .year(1, Some(YEAR))
        .year(2, Some(YEAR));
    let table = [1001, 1002, 1003]
        .into_iter()
        .fold(table, |t, r| t.mode(r, "sub", plain()));
    Arc::new(
        [2001, 2002, 2003]
            .into_iter()
            .fold(table, |t, r| t.mode(r, "sub", tagged())),
    )
}

async fn sub_answer(provider: Arc<TableProvider>) -> crate::error::Result<AvailabilityResponse> {
    let td = tempfile::tempdir().expect("tempdir");
    let mut state = cache_only_state(&td);
    state.providers.set_stub(provider);
    check_availability(&state, &args()).await
}

#[tokio::test]
async fn another_versions_failed_mode_probe_leaves_its_mode_unknown() {
    use crate::scraper::table_provider::Answer;
    let got = sub_answer(modes_answering(
        || Answer::fails(|| crate::error::AniError::Network),
        || true.into(),
    ))
    .await
    .expect("the playing copy answered");
    assert!(got.available);
    let modes: Vec<Option<bool>> = got.versions.iter().map(|v| v.has_mode).collect();
    assert_eq!(modes, vec![None, Some(true)]);
}

#[tokio::test]
async fn a_mode_the_playing_version_leaves_unanswered_is_found_in_another() {
    use crate::scraper::table_provider::Answer;
    let got = sub_answer(modes_answering(
        || true.into(),
        || Answer::fails(|| crate::error::AniError::Upstream { status: 404 }),
    ))
    .await
    .expect("answered");
    assert!(got.available, "the plain copy carries sub");
}

#[tokio::test]
async fn an_absence_one_version_never_answered_is_not_settled() {
    use crate::scraper::table_provider::Answer;
    let got = sub_answer(modes_answering(
        || Answer::fails(|| crate::error::AniError::Upstream { status: 404 }),
        || false.into(),
    ))
    .await
    .expect("answered");
    assert_eq!(
        got.unsettled,
        Some(UnsettledReason::Unanswered),
        "the plain copy never answered: {got:?}"
    );
}

#[tokio::test]
async fn a_plays_stamp_keeps_what_the_probe_said_of_each_version() {
    let td = tempfile::tempdir().expect("tempdir");
    let mut state = cache_only_state(&td);
    state.providers.set_stub(dubbed_plain_copy());
    let probed = check_availability(&state, &args()).await.expect("answered");
    // A play's stamp names the versions without their modes.
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
    assert_eq!(
        row.versions, probed.versions,
        "the probe's per-version answers stand"
    );
}

/// The row standing for `(777, mode)`.
fn row_of(state: &AppState, mode: &str) -> AvailabilityResponse {
    let body = crate::cache::meta_cache_get(&state.cache_pool, &cache_key("777", mode))
        .expect("read")
        .expect("a row");
    serde_json::from_str(&body).expect("row")
}

#[tokio::test]
async fn a_dubbed_plays_stamp_keeps_the_versions_and_their_modes() {
    // A dub resolve stamps the boolean row — its cap is no evidence of
    // the dubbed count — but the versions and what the probe said of
    // each one's dub stand.
    let td = tempfile::tempdir().expect("tempdir");
    let mut state = cache_only_state(&td);
    state.providers.set_stub(dubbed_plain_copy());
    let dub = AvailabilityArgs {
        mode: "dub".into(),
        ..args()
    };
    let probed = check_availability(&state, &dub).await.expect("answered");
    assert_eq!(probed.versions.len(), 2);
    let at_start = RowAtStart::read(&state, Some("777"), "dub").await;
    let versions = both_versions();
    stamp_after_native(
        &state,
        Some("777"),
        "dub",
        &at_start,
        ResolveVerdict::served(crate::provider_set::ProviderSet::TEST_DEFAULT, Some(3), &[])
            .with_versions(&versions),
    )
    .await;
    let row = row_of(&state, "dub");
    assert!(row.available);
    assert_eq!(row.versions, probed.versions);
}

#[tokio::test]
async fn a_count_less_plays_stamp_keeps_the_versions() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    let at_start = RowAtStart::read(&state, Some("777"), "sub").await;
    let versions = both_versions();
    stamp_after_native(
        &state,
        Some("777"),
        "sub",
        &at_start,
        ResolveVerdict::served(crate::provider_set::ProviderSet::TEST_DEFAULT, None, &[])
            .with_versions(&versions),
    )
    .await;
    assert_eq!(row_of(&state, "sub").versions, both_versions());
}

#[tokio::test]
async fn a_replays_stamp_names_the_versions_it_replayed() {
    // Where no positive row stands, a replay writes the boolean
    // positive — with the versions its cached resolution holds.
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    let at_start = RowAtStart::read(&state, Some("777"), "sub").await;
    stamp_after_cache_hit(
        &state,
        Some("777"),
        "sub",
        &at_start,
        crate::provider_set::ProviderSet::TEST_DEFAULT,
        &both_versions(),
    )
    .await;
    assert_eq!(row_of(&state, "sub").versions, both_versions());
}

#[tokio::test]
async fn the_version_that_served_carries_the_mode_in_the_row() {
    // The probe sampled the uncensored copy's dub rows and found none;
    // a dub play then served from it. The stamp says it carries dub.
    let td = tempfile::tempdir().expect("tempdir");
    let mut state = cache_only_state(&td);
    state.providers.set_stub(dubbed_plain_copy());
    let dub = AvailabilityArgs {
        mode: "dub".into(),
        ..args()
    };
    check_availability(&state, &dub).await.expect("answered");
    let at_start = RowAtStart::read(&state, Some("777"), "dub").await;
    let versions = both_versions();
    stamp_after_native(
        &state,
        Some("777"),
        "dub",
        &at_start,
        ResolveVerdict::served(crate::provider_set::ProviderSet::TEST_DEFAULT, Some(3), &[])
            .with_versions(&versions)
            .with_played(Some(VersionKind::Uncensored)),
    )
    .await;
    let modes: Vec<(VersionKind, Option<bool>)> = row_of(&state, "dub")
        .versions
        .iter()
        .map(|v| (v.kind, v.has_mode))
        .collect();
    assert_eq!(
        modes,
        vec![
            (VersionKind::Standard, Some(true)),
            (VersionKind::Uncensored, Some(true)),
        ]
    );
}
