//! Tests for `crate::commands::account`. Extracted via `#[path]` so
//! the dispatcher + helper complexity doesn't pile onto `account.rs`'s
//! CCN budget — per `project_crap_inline_test_gotcha`.

use super::*;
use crate::account::pkce::PkceMethod;

/// Build an `AppState` whose Kitsu client points at `kitsu_uri` (a
/// wiremock server) so id-resolution tests can mock the mappings
/// endpoint. Everything else is throwaway.
#[cfg(test)]
fn state_with_kitsu(kitsu_uri: &str) -> std::sync::Arc<crate::app::AppState> {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    Arc::new(crate::app::AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(crate::app::SCRAPER_CONCURRENCY)),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: AccountWriteLocks::new(),
    })
}

/// Kitsu `/anime/:id?include=mappings` body carrying a MAL mapping.
#[cfg(test)]
const KITSU_MAL_MAPPING_BODY: &str = r#"{
    "data": { "id": "12", "type": "anime", "attributes": { "canonicalTitle": "One Piece" } },
    "included": [{
        "id": "1175",
        "type": "mappings",
        "attributes": { "externalSite": "myanimelist/anime", "externalId": "21" }
    }]
}"#;

#[tokio::test]
async fn resolve_native_media_id_mal_is_the_mapped_mal_id() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::MyAnimeList, "12", None)
        .await
        .expect("resolve ok");
    assert_eq!(got, Some(ProviderMediaId(21)));
}

#[tokio::test]
async fn resolve_native_media_id_anilist_bridges_mal_to_media_id() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Media":{"id":154587}}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::AniList, "12", Some(&anilist.uri()))
        .await
        .expect("resolve ok");
    assert_eq!(got, Some(ProviderMediaId(154587)));
}

#[tokio::test]
async fn resolve_native_media_id_none_when_kitsu_has_no_mal_mapping() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/999"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"999","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::MyAnimeList, "999", None)
        .await
        .expect("resolve ok");
    assert_eq!(got, None);
}

#[tokio::test]
async fn resolve_native_media_id_none_when_anilist_unmapped() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAL_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
        )
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = resolve_native_media_id(&state, ProviderKind::AniList, "12", Some(&anilist.uri()))
        .await
        .expect("resolve ok");
    assert_eq!(got, None);
}

#[tokio::test]
async fn push_progress_skips_unmappable_show_without_writing() {
    // A show Kitsu can't map to MAL → resolve yields None → push_progress
    // returns Ok(None) and never reaches update_entry. The provider
    // built by the dispatcher hits the real MAL host, so a stray write
    // attempt would surface as Network/Upstream, not Ok(None) — proving
    // the short-circuit fires before any upstream call.
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/999"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"999","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let tokens = Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    let got = push_progress(
        &state,
        ProviderKind::MyAnimeList,
        &tokens,
        "999",
        crate::account::provider::EntryUpdate {
            progress_episodes: Some(5),
            ..Default::default()
        },
    )
    .await
    .expect("push ok");
    assert!(got.is_none(), "unmappable show must short-circuit to None");
}

/// Kitsu body for an unmappable show (no MAL mapping) — id 999, empty
/// `included`, so `resolve_native_media_id` yields None and the explicit
/// commands short-circuit before any upstream provider call.
#[cfg(test)]
async fn unmappable_kitsu() -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/999"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"999","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    kitsu
}

#[cfg(test)]
fn test_tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

#[tokio::test]
async fn set_entry_skips_unmappable_show_without_writing() {
    // Explicit edits bypass the monotonic guard, but still short-circuit
    // on an unmappable show before reaching update_entry (the dispatched
    // provider hits the real host, so a stray write would surface as
    // Network/Upstream, not Ok(None)).
    let kitsu = unmappable_kitsu().await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = set_entry(
        &state,
        ProviderKind::MyAnimeList,
        &test_tokens(),
        "999",
        crate::account::provider::EntryUpdate {
            progress_episodes: Some(3),
            ..Default::default()
        },
    )
    .await
    .expect("set ok");
    assert!(got.is_none(), "unmappable show must short-circuit to None");
}

#[tokio::test]
async fn get_entry_returns_none_for_unmappable_show() {
    let kitsu = unmappable_kitsu().await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = get_entry(&state, ProviderKind::MyAnimeList, &test_tokens(), "999")
        .await
        .expect("get ok");
    assert!(got.is_none(), "unmappable show has no current entry");
}

#[tokio::test]
async fn remove_entry_is_noop_for_unmappable_show() {
    let kitsu = unmappable_kitsu().await;
    let state = state_with_kitsu(&kitsu.uri());
    let removed = remove_entry(&state, ProviderKind::MyAnimeList, &test_tokens(), "999")
        .await
        .expect("remove ok");
    assert!(!removed, "unmappable show removes nothing");
}

#[test]
fn account_write_locks_share_one_mutex_per_show() {
    // Codex P2 #3387237642: serialization only works if every call for
    // the same (provider, show) gets the SAME mutex, and distinct shows
    // get distinct ones (so they stay concurrent).
    let locks = AccountWriteLocks::new();
    let a1 = locks.for_show(ProviderKind::MyAnimeList, 21);
    let a2 = locks.for_show(ProviderKind::MyAnimeList, 21);
    assert!(std::sync::Arc::ptr_eq(&a1, &a2), "same show → same mutex");
    let other_show = locks.for_show(ProviderKind::MyAnimeList, 22);
    assert!(
        !std::sync::Arc::ptr_eq(&a1, &other_show),
        "different show → different mutex"
    );
    let other_provider = locks.for_show(ProviderKind::AniList, 21);
    assert!(
        !std::sync::Arc::ptr_eq(&a1, &other_provider),
        "same id, different provider → different mutex"
    );
}

#[test]
fn reconcile_monotonic_clamps_progress_and_reconciles_status() {
    use crate::account::provider::{CurrentEntry, EntryUpdate};
    // The fan-out sends progress-only for non-finale, Completed at the
    // finale. Helpers mirror that.
    let progress_only = |ep| EntryUpdate {
        progress_episodes: Some(ep),
        ..Default::default()
    };
    let watching = |ep| EntryUpdate {
        status: Some(ListStatus::Watching),
        progress_episodes: Some(ep),
        ..Default::default()
    };
    let entry = |status, ep| {
        Some(CurrentEntry {
            status,
            progress_episodes: ep,
        })
    };

    // Codex P2 #3387383171: a progress write to a not-yet-listed show
    // creates it as Watching.
    assert_eq!(
        reconcile_monotonic(progress_only(1), None),
        Some(watching(1))
    );
    // …and promotes a Plan-to-Watch row out of planning.
    assert_eq!(
        reconcile_monotonic(progress_only(6), entry(ListStatus::Planning, 0)),
        Some(watching(6))
    );

    // Codex P2 #3387568872: a planning row already at the same/higher
    // count still promotes to Watching (status-only) — the progress is
    // dropped as non-advancing but the title must leave Watch Later.
    assert_eq!(
        reconcile_monotonic(progress_only(3), entry(ListStatus::Planning, 10)),
        Some(EntryUpdate {
            status: Some(ListStatus::Watching),
            ..Default::default()
        })
    );

    // Codex P2 #3387319861: an advancing write must NOT touch a
    // rewatching (or already-watching) row's status — progress only.
    assert_eq!(
        reconcile_monotonic(progress_only(6), entry(ListStatus::Rewatching, 5)),
        Some(progress_only(6))
    );
    assert_eq!(
        reconcile_monotonic(progress_only(6), entry(ListStatus::Watching, 5)),
        Some(progress_only(6))
    );

    // Codex P1 #3386909281: a non-advancing progress write is dropped
    // entirely — never regress.
    assert_eq!(
        reconcile_monotonic(progress_only(3), entry(ListStatus::Watching, 10)),
        None
    );
    assert_eq!(
        reconcile_monotonic(progress_only(10), entry(ListStatus::Watching, 10)),
        None
    );

    // Codex P2 #3387051891: a Completed write at unchanged progress is
    // still needed — keep the status, drop only the non-advancing
    // progress field.
    let finale = EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(12),
        ..Default::default()
    };
    assert_eq!(
        reconcile_monotonic(finale, entry(ListStatus::Watching, 12)),
        Some(EntryUpdate {
            status: Some(ListStatus::Completed),
            progress_episodes: None,
            ..Default::default()
        })
    );

    // A score-only edit at unchanged progress survives (not a no-op,
    // and no spurious promotion since progress was dropped).
    let rescore = EntryUpdate {
        progress_episodes: Some(5),
        score_0_to_100: Some(90),
        ..Default::default()
    };
    assert_eq!(
        reconcile_monotonic(rescore, entry(ListStatus::Watching, 10)),
        Some(EntryUpdate {
            score_0_to_100: Some(90),
            ..Default::default()
        })
    );
}

#[test]
fn reconcile_monotonic_preserves_rewatching_at_finale() {
    use crate::account::provider::{CurrentEntry, EntryUpdate};
    // Codex P2 #3415780486: the fan-out sends Completed when the user
    // finishes a finished series. If the tracker row is already
    // rewatching/repeating, completing it would clear AniList REPEATING
    // / MAL is_rewatching — contradicting the preserve-rewatching rule.
    // The finale Completed must be dropped for a rewatching row.
    let finale = |ep| EntryUpdate {
        status: Some(ListStatus::Completed),
        progress_episodes: Some(ep),
        ..Default::default()
    };
    let entry = |status, ep| {
        Some(CurrentEntry {
            status,
            progress_episodes: ep,
        })
    };

    // Rewatcher advancing into the finale: progress still flows, but the
    // Completed status is stripped so the row stays rewatching.
    assert_eq!(
        reconcile_monotonic(finale(12), entry(ListStatus::Rewatching, 11)),
        Some(EntryUpdate {
            progress_episodes: Some(12),
            ..Default::default()
        })
    );
    // Rewatcher already at the cap (re-finishing): nothing actionable —
    // progress non-advancing and the Completed is dropped → skip.
    assert_eq!(
        reconcile_monotonic(finale(12), entry(ListStatus::Rewatching, 12)),
        None
    );
    // A genuine Watching → Completed finale is still honored.
    assert_eq!(
        reconcile_monotonic(finale(12), entry(ListStatus::Watching, 11)),
        Some(finale(12))
    );
}

#[test]
fn build_entry_update_rejects_empty_and_unknown_status() {
    // Codex P2 #3381617932: an all-absent update, or a status typo
    // that silently parses to None, would still call update_entry —
    // and since both providers upsert, that creates a list row with
    // upstream defaults. Reject both so a malformed fan-out request
    // is a no-op error, not a phantom "watching" entry.
    assert!(
        build_entry_update(None, None, None).is_err(),
        "all-absent update must be rejected"
    );
    assert!(
        build_entry_update(Some("not_a_status"), Some(5), None).is_err(),
        "unrecognized status must be rejected, not dropped to None"
    );
    let ok = build_entry_update(Some("watching"), Some(5), None).expect("valid update");
    assert_eq!(ok.status, Some(ListStatus::Watching));
    assert_eq!(ok.progress_episodes, Some(5));
    // Progress-only (no status) is a legitimate update.
    let progress_only = build_entry_update(None, Some(7), None).expect("progress-only ok");
    assert!(progress_only.status.is_none());
    assert_eq!(progress_only.progress_episodes, Some(7));
}

#[test]
fn provider_for_kind_dispatches_anilist_and_mal_but_not_inhouse() {
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    let state = Arc::new(crate::app::AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/tmp/ani-cli"),
        bash_path: None,
        bundled_bin: None,
        history_path: PathBuf::from("/tmp/ani-cli/ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(crate::app::SCRAPER_CONCURRENCY)),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::new(reqwest::Client::new()),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: AccountWriteLocks::new(),
    });
    assert!(provider_for_kind(&state, ProviderKind::AniList).is_some());
    assert!(provider_for_kind(&state, ProviderKind::MyAnimeList).is_some());
    assert!(provider_for_kind(&state, ProviderKind::InHouse).is_none());
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

#[test]
fn watch_later_bridge_max_ids_is_a_sane_ceiling() {
    // Codex P1 #3373789621: the route gates on this constant.
    // 500 covers the largest plausible Plan-to-Watch (a heavy
    // listmaker with curated picks tops out around 200-300) and
    // bounds per-request fan-out cost. Pinned here so a future
    // bump is intentional, not a typo.
    assert_eq!(WATCH_LATER_BRIDGE_MAX_IDS, 500);
}

#[test]
fn upsert_cached_entry_writes_through_to_the_cache() {
    // Codex P2 #3412673593: the write-back path upserts the just-synced
    // entry so the Watch Later rail sees the new status without a full
    // resync. Exercise the commands wrapper end-to-end against an
    // in-memory pool.
    use crate::account::provider::{ListEntry, ProviderKind, ProviderMediaId};
    use crate::account::status::ListStatus;
    let state = state_with_kitsu("http://127.0.0.1:0");
    let entry = ListEntry {
        provider: ProviderKind::AniList,
        media_id: ProviderMediaId(5),
        mal_id: Some(5),
        status: ListStatus::Watching,
        progress_episodes: 2,
        score_0_to_100: None,
        updated_at_epoch_s: 0,
        title: "X".into(),
    };
    upsert_cached_entry(&state, ProviderKind::AniList, "u", &entry).unwrap();
    let got = cached_list(&state, ProviderKind::AniList, "u").unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].status, ListStatus::Watching);
}

#[tokio::test]
async fn write_through_after_update_noop_without_entry_or_owner() {
    // Codex P2 #3412673593: best-effort cache write-through. A None
    // entry returns immediately; a provider with no impl (inhouse →
    // me() errors, no network) skips the upsert. Neither writes a row.
    use crate::account::provider::{ListEntry, ProviderKind, ProviderMediaId, Tokens};
    use crate::account::status::ListStatus;
    let state = state_with_kitsu("http://127.0.0.1:0");
    let tokens = Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    };
    write_through_after_update(&state, ProviderKind::AniList, &tokens, None).await;
    assert!(cached_list(&state, ProviderKind::AniList, "u")
        .unwrap()
        .is_empty());

    let entry = ListEntry {
        provider: ProviderKind::InHouse,
        media_id: ProviderMediaId(1),
        mal_id: None,
        status: ListStatus::Watching,
        progress_episodes: 1,
        score_0_to_100: None,
        updated_at_epoch_s: 0,
        title: "Y".into(),
    };
    write_through_after_update(&state, ProviderKind::InHouse, &tokens, Some(&entry)).await;
    assert!(cached_list(&state, ProviderKind::InHouse, "u")
        .unwrap()
        .is_empty());
}
