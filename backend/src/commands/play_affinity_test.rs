//! The play walk against a positive availability row: what a miss
//! reached past the row's provider may and may not do to the row.

use super::*;
use crate::commands::availability::{cached_provider, write_cache};
use crate::scraper::provider::{BrowseHit, EpisodeRef, ProviderId};
use crate::scraper::table_provider::{Asked, TableProvider};
use std::sync::Arc;

/// An `AppState` over an in-memory cache with both providers answered
/// by the given tables. Mirrors the play tests' own builder, which is
/// private to their module.
fn state_over(primary: &Arc<TableProvider>, fallback: &Arc<TableProvider>) -> AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: std::path::PathBuf::from("/tmp/ani-gui/history"),
        providers: crate::provider_set::ProviderSet::listing(vec![
            ProviderId::TestA,
            ProviderId::TestB,
        ])
        .with_stub(Arc::clone(primary))
        .with_stub(Arc::clone(fallback)),
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

/// A fallback that lists the show with one episode, so a play of its
/// second episode is an episode dead end: the show found, the
/// episode not served.
fn fallback_with_one_episode() -> Arc<TableProvider> {
    Arc::new(
        TableProvider::new()
            .answering_as(ProviderId::TestB)
            .search_default(vec![BrowseHit {
                slug: "the-show-7".into(),
                title: "The Show".into(),
                kind: Some("TV".into()),
                jname: None,
            }])
            .listing(
                7,
                vec![EpisodeRef {
                    id: 7001,
                    number: 1,
                    number2: None,
                }],
            ),
    )
}

/// The positive row remembers the fallback, which listed the show; a
/// play of an episode the fallback does not serve moves on to the
/// primary, which has never heard of the show. That clean miss used to
/// be the walk's verdict, and the play persisted it: a negative row
/// named after a healthy primary, backed for its whole lifetime, hiding
/// a title the fallback plays but for one episode. The remembered provider found
/// the show, so its own dead end is the verdict and the row stands.
#[tokio::test]
async fn an_episode_the_remembered_fallback_does_not_serve_does_not_write_the_primarys_miss_over_its_row(
) {
    // A primary that answers every search with nothing.
    let primary = Arc::new(TableProvider::new().answering_as(ProviderId::TestA));
    let fallback = fallback_with_one_episode();
    let state = state_over(&primary, &fallback);
    write_cache(&state, "580", "sub", true, Some(ProviderId::TestB));
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
        matches!(got, Err(crate::error::AniError::EpisodeUnavailable)),
        "the episode is nowhere, and that is the episode's own verdict: {got:?}"
    );
    assert!(
        fallback.count(|a| matches!(a, Asked::Episodes(_))) > 0,
        "the fallback found the show and listed its episodes"
    );
    assert!(
        !primary.asked().is_empty(),
        "the walk moved on to the primary after the dead end"
    );
    assert_eq!(
        cached_provider(&state, "580", "sub"),
        Some(ProviderId::TestB),
        "the positive row stands: the fallback found the show"
    );
}
