//! The gate against a mock AniList that keeps AniList's own rule: past
//! its limit in a window it answers 429 with `Retry-After`, and every
//! request it serves is timed, so a test can say what reached it when.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wiremock::matchers::method;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use super::{in_background, share_for_tests, Pace, ANILIST};
use crate::account::provider::{EntryUpdate, ProviderMediaId, Tokens, UserListProvider};
use crate::error::AniError;
use crate::meta::anilist::{mal_id_for_media_id, post_graphql_public, trending};
use crate::meta::anilist_media::{banner_for_media, MediaRef};
use crate::meta::anilist_streaming_eps::streaming_listing_for_media;
use crate::meta::anilist_user::AniListProvider;

/// One body every public parser reads: an empty trending page, and a
/// `Media` AniList does not have.
const ANY_BODY: &str = r#"{"data":{"Page":{"media":[]},"Media":null}}"#;

/// A mock AniList that serves `limit` requests in any `window` and
/// answers the rest 429 with `retry_after` seconds, like the real one
/// — or, built by [`Limited::refusing_first`], refuses its first
/// request alone.
#[derive(Clone)]
struct Limited {
    limit: usize,
    window: Duration,
    retry_after: u64,
    refuse_first: bool,
    hits: Arc<Mutex<Vec<(Instant, u16)>>>,
}

impl Limited {
    fn new(limit: usize, window: Duration, retry_after: u64) -> Self {
        Self {
            limit,
            window,
            retry_after,
            refuse_first: false,
            hits: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// The first request answered 429 with `retry_after` seconds, every
    /// later one served.
    fn refusing_first(retry_after: u64) -> Self {
        Self {
            refuse_first: true,
            ..Self::new(usize::MAX, Duration::ZERO, retry_after)
        }
    }

    fn hits(&self) -> Vec<(Instant, u16)> {
        self.hits.lock().unwrap().clone()
    }

    fn refusals(&self) -> usize {
        self.hits().iter().filter(|(_, s)| *s == 429).count()
    }
}

impl Respond for Limited {
    fn respond(&self, _req: &Request) -> ResponseTemplate {
        let now = Instant::now();
        let mut hits = self.hits.lock().unwrap();
        let recent = hits
            .iter()
            .filter(|(at, _)| now.duration_since(*at) < self.window)
            .count();
        let refused = if self.refuse_first {
            hits.is_empty()
        } else {
            recent >= self.limit
        };
        let status = if refused { 429 } else { 200 };
        hits.push((now, status));
        if status == 429 {
            ResponseTemplate::new(429).insert_header("retry-after", self.retry_after.to_string())
        } else {
            ResponseTemplate::new(200).set_body_string(ANY_BODY)
        }
    }
}

async fn serve(responder: Limited) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(responder)
        .mount(&server)
        .await;
    server
}

fn tokens() -> Tokens {
    Tokens {
        access_token: "t".into(),
        refresh_token: None,
        expires_at_epoch_s: i64::MAX,
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn public(uri: &str) -> crate::error::Result<bytes::Bytes> {
    post_graphql_public(&client(), uri, &serde_json::json!({})).await
}

fn assert_rate_limited<T: std::fmt::Debug>(got: &crate::error::Result<T>) {
    assert!(
        matches!(got, Err(AniError::RateLimited { .. })),
        "expected a rate limit, got {got:?}"
    );
}

// --- The cool-down --------------------------------------------------

#[tokio::test]
async fn after_a_429_nothing_reaches_anilist_until_its_retry_after_passes() {
    // AniList answers the first request 429 with a two-second wait.
    // That answer is reported as the rate limit it is; a caller who
    // cannot wait is refused without a request; one that can lands
    // after the window, and not before.
    let anilist = Limited::refusing_first(2);
    let server = serve(anilist.clone()).await;
    let _shared = share_for_tests(
        &server.uri(),
        Pace {
            min_gap: Duration::ZERO,
            interactive_wait: Duration::ZERO,
            background_wait: Duration::from_secs(5),
            ..ANILIST
        },
    );

    let first = public(&server.uri()).await;
    assert!(
        matches!(
            first,
            Err(AniError::RateLimited {
                retry_after_secs: Some(2)
            })
        ),
        "a genuine 429 is reported with its wait: {first:?}"
    );
    assert_eq!(anilist.hits().len(), 1);

    let impatient = public(&server.uri()).await;
    assert_rate_limited(&impatient);
    assert_eq!(anilist.hits().len(), 1, "nothing is sent inside the window");

    let _ = in_background(public(&server.uri())).await;
    let hits = anilist.hits();
    assert_eq!(hits.len(), 2, "the patient caller is sent once: {hits:?}");
    assert!(
        hits[1].0.duration_since(hits[0].0) >= Duration::from_secs(2),
        "the second request left inside the window: {:?}",
        hits[1].0.duration_since(hits[0].0)
    );
}

#[tokio::test]
async fn a_503_naming_a_wait_cools_down_like_a_429() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503).insert_header("retry-after", "30"))
        .expect(1)
        .mount(&server)
        .await;
    let _shared = share_for_tests(&server.uri(), ANILIST);
    let first = public(&server.uri()).await;
    assert!(
        matches!(first, Err(AniError::Upstream { status: 503 })),
        "{first:?}"
    );
    assert_rate_limited(&public(&server.uri()).await);
}

#[tokio::test]
async fn no_requests_left_in_the_window_holds_the_next_one_back() {
    // AniList counts down `X-RateLimit-Remaining`; at zero the next
    // request is the one it refuses with a fresh minute's timeout.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-ratelimit-remaining", "0")
                .set_body_string(ANY_BODY),
        )
        .expect(1)
        .mount(&server)
        .await;
    let _shared = share_for_tests(&server.uri(), ANILIST);
    public(&server.uri())
        .await
        .expect("the answer itself stands");
    assert_rate_limited(&public(&server.uri()).await);
}

#[tokio::test]
async fn the_signed_in_account_waits_out_the_same_cool_down() {
    // A public read's 429 holds back the account's reads and writes:
    // AniList counts this client, not the token.
    let anilist = Limited::new(0, Duration::from_millis(1), 60);
    let server = serve(anilist.clone()).await;
    let _shared = share_for_tests(&server.uri(), ANILIST);
    assert_rate_limited(&public(&server.uri()).await);

    let provider = AniListProvider::with_bases(client(), server.uri(), server.uri());
    let tokens = tokens();
    let me = provider.me(&tokens).await;
    assert_rate_limited(&me);
    assert_eq!(anilist.hits().len(), 1, "the account request was sent");
}

#[tokio::test]
async fn a_write_held_back_by_the_cool_down_lands_after_it() {
    // A list write that meets the cool-down is refused with the wait
    // left — no request spent — and the same write lands once the
    // window has passed.
    let anilist = Limited::refusing_first(2);
    let server = serve(anilist.clone()).await;
    let _shared = share_for_tests(
        &server.uri(),
        Pace {
            min_gap: Duration::ZERO,
            interactive_wait: Duration::ZERO,
            ..ANILIST
        },
    );
    assert_rate_limited(&public(&server.uri()).await);

    let provider = AniListProvider::with_bases(client(), server.uri(), server.uri());
    let tokens = tokens();
    let update = EntryUpdate {
        progress_episodes: Some(3),
        ..EntryUpdate::default()
    };
    let held = provider
        .update_entry(&tokens, ProviderMediaId(7), update.clone())
        .await;
    let Err(AniError::RateLimited {
        retry_after_secs: Some(wait),
    }) = held
    else {
        panic!("expected the write held back with its wait, got {held:?}");
    };
    assert!(wait <= 2, "{wait}");
    assert_eq!(anilist.hits().len(), 1, "the held write was sent");

    tokio::time::sleep(Duration::from_secs(wait)).await;
    let _ = provider
        .update_entry(&tokens, ProviderMediaId(7), update)
        .await;
    assert_eq!(anilist.hits().len(), 2, "the write never reached AniList");
}

// --- Pacing ---------------------------------------------------------

/// A cold home page's AniList reads, at the speed the page asks for
/// them: the trending feed, then a banner, an episode listing and an
/// id bridge per card, the fills in the background.
async fn cold_home(uri: &str, cards: u32) -> Vec<bool> {
    let client = client();
    let mut calls: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>> =
        Vec::new();
    {
        let client = client.clone();
        let uri = uri.to_owned();
        calls.push(Box::pin(async move {
            trending(&client, 25, Some(&uri)).await.is_ok()
        }));
    }
    for id in 0..cards {
        let (c1, c2, c3) = (client.clone(), client.clone(), client.clone());
        let (u1, u2, u3) = (uri.to_owned(), uri.to_owned(), uri.to_owned());
        calls.push(Box::pin(in_background(async move {
            banner_for_media(&c1, MediaRef::Mal(id), Some(&u1))
                .await
                .is_ok()
        })));
        calls.push(Box::pin(in_background(async move {
            streaming_listing_for_media(&c2, MediaRef::Mal(id), Some(&u2))
                .await
                .is_ok()
        })));
        calls.push(Box::pin(async move {
            mal_id_for_media_id(&c3, id, Some(&u3)).await.is_ok()
        }));
    }
    futures_util::future::join_all(calls).await
}

#[tokio::test]
async fn a_cold_home_page_stays_under_anilist_limit() {
    // AniList's rule — 429 past its 30th request in a window — on a
    // window shortened to three seconds, against AniList's own pace
    // over the same window. Sixteen cards make 49 requests: past the
    // limit, so an unpaced burst is refused, and within two windows,
    // so a paced one is served in full.
    let window = Duration::from_secs(3);
    let anilist = Limited::new(30, window, 3);
    let server = serve(anilist.clone()).await;
    let _shared = share_for_tests(
        &server.uri(),
        Pace {
            window,
            min_gap: Duration::ZERO,
            interactive_wait: Duration::from_secs(10),
            background_wait: Duration::from_secs(10),
            ..ANILIST
        },
    );

    let answered = cold_home(&server.uri(), 16).await;

    assert_eq!(anilist.refusals(), 0, "AniList refused the burst");
    assert!(answered.iter().all(|ok| *ok), "{answered:?}");
    let hits = anilist.hits();
    assert_eq!(hits.len(), answered.len());
    for &(at, _) in &hits {
        let in_window = hits
            .iter()
            .filter(|(s, _)| *s <= at && at.duration_since(*s) < window)
            .count();
        assert!(in_window <= ANILIST.capacity, "{in_window} in one window");
    }
}

#[tokio::test]
async fn background_fills_leave_the_reserve_to_the_user() {
    // Background fills have spent all of the window but the reserve:
    // the next fill is held back, while the user's own read goes now.
    let anilist = Limited::new(100, Duration::from_secs(60), 60);
    let server = serve(anilist.clone()).await;
    let _shared = share_for_tests(
        &server.uri(),
        Pace {
            capacity: 4,
            reserve: 2,
            min_gap: Duration::ZERO,
            interactive_wait: Duration::ZERO,
            background_wait: Duration::ZERO,
            ..ANILIST
        },
    );
    for _ in 0..2 {
        in_background(public(&server.uri()))
            .await
            .expect("inside the background share");
    }
    assert_rate_limited(&in_background(public(&server.uri())).await);
    public(&server.uri())
        .await
        .expect("the reserve is the user's");
    assert_eq!(anilist.hits().len(), 3);
}

#[test]
fn a_gate_is_kept_by_host_and_port() {
    use super::pace::authority;
    assert_eq!(
        authority("https://graphql.anilist.co"),
        "graphql.anilist.co:443"
    );
    assert_eq!(authority("http://127.0.0.1:5000/x"), "127.0.0.1:5000");
    assert_eq!(authority("not a url"), "not a url");
}

#[tokio::test]
async fn a_429_without_a_wait_holds_anilist_back_for_its_documented_minute() {
    // AniList documents a one-minute timeout. A 429 that names no wait
    // is reported as it came — no wait to pass on — and nothing is sent
    // for that minute.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429))
        .expect(1)
        .mount(&server)
        .await;
    let _shared = share_for_tests(&server.uri(), ANILIST);
    let first = public(&server.uri()).await;
    assert!(
        matches!(
            first,
            Err(AniError::RateLimited {
                retry_after_secs: None
            })
        ),
        "{first:?}"
    );
    let held = public(&server.uri()).await;
    assert!(
        matches!(held, Err(AniError::RateLimited { retry_after_secs: Some(s) }) if (59..=60).contains(&s)),
        "{held:?}"
    );
}
