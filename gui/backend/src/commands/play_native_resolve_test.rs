use super::*;
use crate::anicli::parser::ProgressLine;
use crate::scraper::anidb::{AnidbClient, AnidbFetch, FetchResponse};
use std::sync::Mutex;

/// Scriptable provider: browse responses keyed by query substring,
/// one shared episodes/languages/embed/master catalogue, and a URL
/// log so tests can assert how far the walk went.
struct Provider {
    /// `(query substring, browse HTML)` — first match wins. A `!`
    /// body means "answer 403 with the interstitial".
    browse: &'static [(&'static str, &'static str)],
    log: Mutex<Vec<String>>,
}

impl Provider {
    fn new(browse: &'static [(&'static str, &'static str)]) -> Self {
        Self {
            browse,
            log: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<String> {
        self.log.lock().expect("log").clone()
    }
}

fn browse_page(entries: &[(&str, &str)]) -> String {
    entries
        .iter()
        .map(|(slug, title)| format!("<a href=\"/anime/{slug}\"><img alt=\"{title}\"/></a>\n"))
        .collect()
}

#[async_trait::async_trait]
impl AnidbFetch for Provider {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        self.log.lock().expect("log").push(url.to_string());
        if let Some(q) = url.split("browse?q=").nth(1) {
            for (needle, body) in self.browse {
                if q.contains(needle) {
                    if *body == "!" {
                        return Ok(FetchResponse {
                            status: 403,
                            body: "Just a moment".into(),
                        });
                    }
                    return Ok(FetchResponse {
                        status: 200,
                        body: (*body).to_string(),
                    });
                }
            }
            return Ok(FetchResponse {
                status: 200,
                body: String::new(),
            });
        }
        if url.contains("/api/frontend/anime/77/episodes") {
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"episodes":[{"id":701,"number":1},{"id":702,"number":2},{"id":703,"number":3}]}"#
                    .into(),
            });
        }
        if url.contains("/api/frontend/anime/88/episodes") {
            // A continuation entry: the provider keeps the franchise's
            // cumulative numbering, so this two-episode cour lists 41
            // and 42 (the TYBW fourth-cour shape, captured live).
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"episodes":[{"id":8841,"number":41},{"id":8842,"number":42}]}"#.into(),
            });
        }
        if url.contains("/api/frontend/episode/8841/languages") {
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"languages":[{"code":"jpn","embed_url":"https://embed.example/e/s1"}]}"#
                    .into(),
            });
        }
        if url.contains("/api/frontend/episode/702/languages") {
            return Ok(FetchResponse {
                status: 200,
                body: r#"{"languages":[{"code":"jpn","embed_url":"https://embed.example/e/x"}]}"#
                    .into(),
            });
        }
        if url.contains("embed.example") {
            return Ok(FetchResponse {
                status: 200,
                body: "player.setup({ file: 'https://cdn.example/x/master.m3u8' });".into(),
            });
        }
        Ok(FetchResponse {
            status: 404,
            body: String::new(),
        })
    }
}

// The show catalogue every happy-path test shares: slug the-show-77,
// three episodes, jpn embed on episode 2.
const THE_SHOW: &str = "the-show-77\" alt-marker";

fn the_show_browse() -> &'static str {
    // Leaked once per process; fine for tests.
    Box::leak(browse_page(&[("the-show-77", "The Show")]).into_boxed_str())
}

async fn run(
    provider: &Provider,
    title: &str,
    alts: &[String],
    episode: &str,
    expected: Option<u32>,
) -> (
    std::result::Result<NativeResolved, NativeError>,
    Vec<ProgressLine>,
) {
    let client = AnidbClient::new(ProviderRef(provider));
    let mut events = Vec::new();
    let got = resolve_native(
        &client,
        None,
        crate::scraper::gate::ScrapePriority::Interactive,
        NativeResolveRequest {
            title,
            alt_titles: alts,
            episode,
            mode: "sub",
            expected_count: expected,
            year: None,
            subtype: None,
        },
        &mut |p| events.push(p),
    )
    .await;
    (got, events)
}

/// Borrowing adapter so one Provider serves a whole test.
struct ProviderRef<'a>(&'a Provider);

#[async_trait::async_trait]
impl AnidbFetch for ProviderRef<'_> {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        self.0.get(url).await
    }
}

#[tokio::test]
async fn canonical_title_resolves_to_the_master_url() {
    let _ = THE_SHOW;
    let provider = Provider::new(Box::leak(Box::new([("the+show", the_show_browse())])));
    let (got, events) = run(&provider, "the show", &[], "2", Some(3)).await;
    let resolved = got.expect("resolved");
    assert_eq!(resolved.slug, "the-show-77");
    assert_eq!(resolved.title, "The Show");
    assert_eq!(resolved.master_url, "https://cdn.example/x/master.m3u8");
    assert_eq!(resolved.episode_cap, Some(3));
    // Progress trail: a search banner, then the links-fetched marker
    // the overlay renders as the provider step.
    assert!(matches!(events.first(), Some(ProgressLine::Banner { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, ProgressLine::LinksFetched { provider } if provider == "anidb.app")));
}

#[tokio::test]
async fn alt_title_recovers_when_canonical_finds_nothing() {
    let provider = Provider::new(Box::leak(Box::new([("romanized", the_show_browse())])));
    let alts = vec!["romanized name".to_string()];
    let (got, _) = run(&provider, "english name", &alts, "2", Some(3)).await;
    assert_eq!(got.expect("resolved").slug, "the-show-77");
}

#[tokio::test]
async fn a_rejected_pool_keeps_the_walk_going() {
    // Canonical returns only a far-off-count sibling; the alt carries
    // the real show. The picker's rejection must not end the walk.
    let wrong = Box::leak(browse_page(&[("wrong-99", "Wrong")]).into_boxed_str());
    let provider = Provider::new(Box::leak(Box::new([
        ("english", &*wrong),
        ("romanized", the_show_browse()),
    ])));
    let alts = vec!["romanized".to_string()];
    let (got, _) = run(&provider, "english", &alts, "2", Some(3)).await;
    assert_eq!(got.expect("resolved").slug, "the-show-77");
}

#[tokio::test]
async fn an_upstream_block_stops_the_walk_as_transient() {
    let provider = Provider::new(Box::leak(Box::new([("english", "!"), ("alt", "!")])));
    let alts = vec!["alt".to_string()];
    let (got, _) = run(&provider, "english", &alts, "2", Some(3)).await;
    let err = got.expect_err("blocked");
    assert!(!err.clean_miss);
    assert!(matches!(
        err.error,
        AniError::Network | AniError::Upstream { .. }
    ));
    // The walk stopped at the first refusal: one browse request only.
    let browses = provider
        .requests()
        .iter()
        .filter(|u| u.contains("browse?q="))
        .count();
    assert_eq!(browses, 1);
}

#[tokio::test]
async fn a_clean_all_empty_walk_is_the_only_persistable_miss() {
    let provider = Provider::new(Box::leak(Box::new([])));
    let alts = vec!["other".to_string()];
    let (got, _) = run(&provider, "nothing", &alts, "2", Some(3)).await;
    let err = got.expect_err("no match");
    assert!(err.clean_miss);
    assert!(matches!(err.error, AniError::NoResults));
}

#[tokio::test]
async fn an_unlisted_episode_is_a_dead_end_not_absence() {
    let provider = Provider::new(Box::leak(Box::new([("the+show", the_show_browse())])));
    let (got, _) = run(&provider, "the show", &[], "9", Some(3)).await;
    let err = got.expect_err("unlisted episode");
    assert!(!err.clean_miss);
    assert!(matches!(err.error, AniError::NoResults));
}

#[tokio::test]
async fn a_mode_without_embed_is_a_dead_end_not_absence() {
    let provider = Provider::new(Box::leak(Box::new([("the+show", the_show_browse())])));
    let client = AnidbClient::new(ProviderRef(&provider));
    let mut sink = |_p: ProgressLine| {};
    let got = resolve_native(
        &client,
        None,
        crate::scraper::gate::ScrapePriority::Interactive,
        NativeResolveRequest {
            title: "the show",
            alt_titles: &[],
            episode: "2",
            mode: "dub",
            expected_count: Some(3),
            year: None,
            subtype: None,
        },
        &mut sink,
    )
    .await;
    let err = got.expect_err("no dub embed");
    assert!(!err.clean_miss);
}

#[tokio::test]
async fn a_continuation_entry_maps_kitsu_numbers_onto_its_own() {
    // anidb.app numbers a later cour cumulatively: TYBW's fourth part
    // lists episodes 41 and 42, while Kitsu (and so every episode the
    // UI can request) numbers the same cour 1..13. Episode "1" must
    // land on the entry's first listed episode, and the cap must come
    // back in the request's numbering — 2 aired — not the provider's
    // raw 42, or the strip unlocks eleven episodes that don't exist.
    let sequel = Box::leak(browse_page(&[("the-sequel-88", "The Sequel")]).into_boxed_str());
    let provider = Provider::new(Box::leak(Box::new([("sequel", &*sequel)])));
    let (got, _) = run(&provider, "the sequel", &[], "1", None).await;
    let resolved = got.expect("episode 1 resolves through the offset");
    assert_eq!(resolved.slug, "the-sequel-88");
    assert_eq!(resolved.master_url, "https://cdn.example/x/master.m3u8");
    assert_eq!(resolved.episode_cap, Some(2));
}

#[tokio::test]
async fn a_continuation_entry_still_rejects_numbers_past_its_tail() {
    // Kitsu episode 3 would map to provider 43 — not aired, not
    // listed. The dead-end classification must survive the offset.
    let sequel = Box::leak(browse_page(&[("the-sequel-88", "The Sequel")]).into_boxed_str());
    let provider = Provider::new(Box::leak(Box::new([("sequel", &*sequel)])));
    let (got, _) = run(&provider, "the sequel", &[], "3", None).await;
    let err = got.expect_err("episode 3 has not aired");
    assert!(!err.clean_miss);
    assert!(matches!(err.error, AniError::NoResults));
}

#[test]
fn breaker_outcome_treats_answered_verdicts_as_health() {
    use crate::scraper::gate::ScrapeOutcome;
    // NoResults only arises from provider-answered verdicts — a
    // rejected pool, an absent episode, a sub-only show asked for
    // dub. The provider answering is breaker health whatever the
    // answer was; recording it as failure opens the breaker on
    // repeated dub requests for sub-only shows and refuses unrelated
    // background traffic.
    let absent_episode = NativeError {
        error: AniError::NoResults,
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(absent_episode)),
        ScrapeOutcome::Success
    ));
    // Weather stays distress.
    let refusal = NativeError {
        error: AniError::Upstream { status: 403 },
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(refusal)),
        ScrapeOutcome::Failure
    ));
    let limited = NativeError {
        error: AniError::RateLimited {
            retry_after_secs: Some(30),
        },
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(limited)),
        ScrapeOutcome::RateLimited { .. }
    ));
    let transport = NativeError {
        error: AniError::Network,
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(transport)),
        ScrapeOutcome::Failure
    ));
}
