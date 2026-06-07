//! Tests for `crate::commands::account`. Extracted via `#[path]` so
//! the dispatcher + helper complexity doesn't pile onto `account.rs`'s
//! CCN budget — per `project_crap_inline_test_gotcha`.

use super::*;
use crate::account::pkce::PkceMethod;

#[test]
fn provider_for_kind_returns_some_for_anilist_only_in_pr_1() {
    let client = reqwest::Client::new();
    assert!(provider_for_kind(ProviderKind::AniList, client.clone()).is_some());
    assert!(provider_for_kind(ProviderKind::MyAnimeList, client.clone()).is_none());
    assert!(provider_for_kind(ProviderKind::InHouse, client).is_none());
}

#[test]
fn pkce_for_kind_picks_method_per_provider() {
    assert_eq!(
        pkce_for_kind(ProviderKind::AniList).method,
        PkceMethod::S256
    );
    assert_eq!(
        pkce_for_kind(ProviderKind::MyAnimeList).method,
        PkceMethod::Plain
    );
    assert_eq!(
        pkce_for_kind(ProviderKind::InHouse).method,
        PkceMethod::S256
    );
}

#[test]
fn status_snake_round_trips_every_variant() {
    for s in [
        ListStatus::Planning,
        ListStatus::Watching,
        ListStatus::Completed,
        ListStatus::Paused,
        ListStatus::Dropped,
        ListStatus::Rewatching,
    ] {
        assert_eq!(status_from_snake(status_to_snake(s)), Some(s));
    }
}

#[test]
fn status_from_snake_returns_none_for_unknown() {
    assert_eq!(status_from_snake(""), None);
    assert_eq!(status_from_snake("Planning"), None);
    assert_eq!(status_from_snake("plan_to_watch"), None);
}

#[test]
fn tokens_from_bearer_drops_expiry_and_refresh() {
    let t = tokens_from_bearer("xyz");
    assert_eq!(t.access_token, "xyz");
    assert!(t.refresh_token.is_none());
    assert_eq!(t.expires_at_epoch_s, 0);
}
