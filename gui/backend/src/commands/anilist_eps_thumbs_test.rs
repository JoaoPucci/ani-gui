use super::*;

fn ep_with(num: u32, thumb: Option<&str>) -> KitsuEpisode {
    KitsuEpisode {
        id: format!("e{num}"),
        canonical_title: Some(format!("Ep {num}")),
        season_number: Some(1),
        number: Some(num),
        relative_number: Some(num),
        length: None,
        synopsis: None,
        airdate: None,
        thumbnail: thumb.map(|t| KitsuEpisodeThumbnail {
            original: Some(t.to_string()),
        }),
    }
}

#[test]
fn merge_thumbs_keeps_kitsu_thumb_when_present() {
    let eps = vec![ep_with(1, Some("https://kitsu.cdn/1.jpg"))];
    let mut anilist = HashMap::new();
    anilist.insert(1u32, "https://crunchyroll.cdn/1.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert_eq!(
        merged[0]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://kitsu.cdn/1.jpg"
    );
}

#[test]
fn merge_thumbs_fills_null_thumb_from_anilist() {
    let eps = vec![ep_with(54, None)];
    let mut anilist = HashMap::new();
    anilist.insert(54u32, "https://crunchyroll.cdn/54.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert_eq!(
        merged[0]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://crunchyroll.cdn/54.jpg"
    );
}

#[test]
fn merge_thumbs_leaves_null_thumb_when_anilist_missing() {
    // The "both gap" case (One Piece eps 54-61, 131-1010+): Kitsu
    // null, AniList also has nothing. Placeholder still renders.
    let eps = vec![ep_with(131, None)];
    let anilist = HashMap::<u32, String>::new();
    let merged = merge_thumbs(eps, &anilist);
    assert!(merged[0].thumbnail.is_none());
}

#[test]
fn merge_thumbs_handles_mixed_pool() {
    // Realistic shape from a One Piece-shaped probe: ep 53 Kitsu
    // present, ep 54 Kitsu null + AniList null, ep 62 Kitsu null +
    // AniList present, ep 130 ditto.
    let eps = vec![
        ep_with(53, Some("https://kitsu.cdn/53.jpg")),
        ep_with(54, None),
        ep_with(62, None),
        ep_with(130, None),
    ];
    let mut anilist = HashMap::new();
    anilist.insert(62u32, "https://cr.cdn/62.jpg".to_string());
    anilist.insert(130u32, "https://cr.cdn/130.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert_eq!(
        merged[0]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://kitsu.cdn/53.jpg"
    );
    assert!(merged[1].thumbnail.is_none());
    assert_eq!(
        merged[2]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://cr.cdn/62.jpg"
    );
    assert_eq!(
        merged[3]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://cr.cdn/130.jpg"
    );
}

#[test]
fn merge_thumbs_skips_eps_without_number() {
    // Kitsu sometimes serves episodes with null `number` (mostly
    // movies / specials in a TV show's listing). No number → no
    // way to match an AniList entry; pass through unchanged.
    let mut ep = ep_with(1, None);
    ep.number = None;
    let eps = vec![ep];
    let mut anilist = HashMap::new();
    anilist.insert(1u32, "https://cr.cdn/1.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert!(merged[0].thumbnail.is_none());
}

#[test]
fn merge_thumbs_replaces_thumbnail_with_only_null_original() {
    // Kitsu sometimes serves `thumbnail: { original: null }` —
    // shape-wise present, content-wise empty. Treat it the same as
    // missing thumb and backfill from AniList.
    let mut ep = ep_with(1, None);
    ep.thumbnail = Some(KitsuEpisodeThumbnail { original: None });
    let eps = vec![ep];
    let mut anilist = HashMap::new();
    anilist.insert(1u32, "https://cr.cdn/1.jpg".to_string());
    let merged = merge_thumbs(eps, &anilist);
    assert_eq!(
        merged[0]
            .thumbnail
            .as_ref()
            .unwrap()
            .original
            .as_deref()
            .unwrap(),
        "https://cr.cdn/1.jpg"
    );
}

#[test]
fn cache_anilist_eps_thumbs_caches_success_map_by_kitsu_id() {
    let pool = crate::cache::open_in_memory().expect("in-mem pool");
    let mut map = HashMap::new();
    map.insert(1u32, "https://cr.cdn/1.jpg".to_string());
    cache_anilist_eps_thumbs(&pool, "12", &Ok(map.clone()));
    let body = meta_cache_get(&pool, "anilist:eps-thumbs:v1:k12")
        .expect("cache read")
        .expect("cache hit");
    let back: HashMap<u32, String> = serde_json::from_str(&body).expect("json");
    assert_eq!(back, map);
}

#[test]
fn cache_anilist_eps_thumbs_negative_caches_empty_on_error() {
    // The rate-limit-burst story: AniList 429s on cold load, we
    // negative-cache an empty map so the next 5 minutes of
    // navigation see a cache hit instead of re-burning the budget.
    // The kitsu_id-level key means a subsequent hit also avoids
    // re-running the Kitsu /mappings lookup.
    let pool = crate::cache::open_in_memory().expect("in-mem pool");
    cache_anilist_eps_thumbs(&pool, "12", &Err(()));
    let body = meta_cache_get(&pool, "anilist:eps-thumbs:v1:k12")
        .expect("cache read")
        .expect("cache hit — empty is still a hit");
    let back: HashMap<u32, String> = serde_json::from_str(&body).expect("json");
    assert!(back.is_empty());
}

#[test]
fn anilist_eps_thumbs_key_namespace_separates_kitsu_ids() {
    assert_eq!(anilist_eps_thumbs_key("12"), "anilist:eps-thumbs:v1:k12");
    assert_eq!(anilist_eps_thumbs_key("818"), "anilist:eps-thumbs:v1:k818");
    assert_ne!(anilist_eps_thumbs_key("12"), anilist_eps_thumbs_key("818"));
}

/// Build a state wired to a nowhere-Kitsu so any network call in
/// `thumbs_for_show`'s miss path errors out — exercising the
/// happy hit path without standing up a wiremock.
fn state_for_cache_only_tests() -> AppState {
    use crate::app::SCRAPER_CONCURRENCY;
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Semaphore;
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        ani_cli_path: PathBuf::from("/x"),
        bash_path: None,
        bundled_bin: None,
        history_path: PathBuf::from("/y/ani-hsts"),
        scraper_slots: Arc::new(Semaphore::new(SCRAPER_CONCURRENCY)),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
    }
}

#[test]
fn needs_backfill_false_when_every_ep_has_thumb() {
    // The all-Kitsu-thumbs case (e.g. an Attack on Titan page where
    // Kitsu's CDN already covers every ep). The AniList fetch would
    // be unconditional otherwise, burning a Kitsu /mappings round-trip
    // + an AniList rate-limit slot on cold cache with no visible win.
    let eps = vec![
        ep_with(1, Some("https://kitsu.cdn/1.jpg")),
        ep_with(2, Some("https://kitsu.cdn/2.jpg")),
        ep_with(3, Some("https://kitsu.cdn/3.jpg")),
    ];
    assert!(!needs_backfill(&eps));
}

#[test]
fn needs_backfill_true_when_at_least_one_ep_missing_thumb() {
    // One Piece-shaped case: most early eps have Kitsu thumbs, ep 54+
    // are null. AniList lookup is worth running.
    let eps = vec![
        ep_with(1, Some("https://kitsu.cdn/1.jpg")),
        ep_with(54, None),
    ];
    assert!(needs_backfill(&eps));
}

#[test]
fn needs_backfill_true_when_thumb_object_present_but_original_null() {
    // Kitsu sometimes serves `thumbnail: { original: null }` —
    // shape-wise present, content-wise empty. Same as missing.
    let mut ep = ep_with(1, None);
    ep.thumbnail = Some(KitsuEpisodeThumbnail { original: None });
    assert!(needs_backfill(&[ep]));
}

#[test]
fn needs_backfill_skips_eps_without_number() {
    // Eps without a `number` can't be merged regardless of AniList's
    // map (the merge keys by ep number). If those are the only ones
    // missing thumbs, there's nothing to backfill — skip the fetch.
    let mut ep = ep_with(1, None);
    ep.number = None;
    let eps = vec![ep, ep_with(2, Some("https://kitsu.cdn/2.jpg"))];
    assert!(!needs_backfill(&eps));
}

#[test]
fn needs_backfill_false_for_empty_page() {
    // Out-of-range pages return zero episodes. No backfill needed.
    assert!(!needs_backfill(&[]));
}

#[tokio::test]
async fn thumbs_for_show_returns_cached_map_without_network() {
    // The hot path: cache is warm, hit returns instantly with no
    // Kitsu /mappings call and no AniList round-trip. The state's
    // kitsu base is pointed at an unbound port so any miss-path
    // attempt would error visibly — the test passes precisely
    // because the cache-hit branch short-circuits before that.
    let state = state_for_cache_only_tests();
    let mut prefilled = HashMap::new();
    prefilled.insert(1u32, "https://x.cdn/1.jpg".to_string());
    prefilled.insert(2u32, "https://x.cdn/2.jpg".to_string());
    cache_anilist_eps_thumbs(&state.cache_pool, "777", &Ok(prefilled.clone()));
    let got = thumbs_for_show(&state, "777").await;
    assert_eq!(got, prefilled);
}

#[tokio::test]
async fn thumbs_for_show_negative_caches_on_cache_miss_with_unreachable_kitsu() {
    // The miss path: nothing in cache, Kitsu mappings lookup errors
    // (state.kitsu points at an unbound port). `fetch_anilist_eps_thumbs`
    // returns Err, `cache_anilist_eps_thumbs` writes an empty map to
    // the negative cache, and the caller sees the empty result. A
    // second call hits cache and returns instantly.
    let state = state_for_cache_only_tests();
    let first = thumbs_for_show(&state, "999").await;
    assert!(first.is_empty());
    // Verify the negative cache was written.
    let body = meta_cache_get(&state.cache_pool, "anilist:eps-thumbs:v1:k999")
        .expect("cache read")
        .expect("negative cache hit");
    let cached: HashMap<u32, String> = serde_json::from_str(&body).expect("json");
    assert!(cached.is_empty());
}

#[tokio::test]
async fn thumbs_for_show_returns_empty_on_cached_negative() {
    // The negative-cached branch: an earlier failed lookup wrote
    // an empty map. Subsequent calls hit cache and return empty
    // without retrying the network.
    let state = state_for_cache_only_tests();
    cache_anilist_eps_thumbs(&state.cache_pool, "778", &Err(()));
    let got = thumbs_for_show(&state, "778").await;
    assert!(got.is_empty());
}
