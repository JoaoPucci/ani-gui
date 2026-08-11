use super::mode_present;
use crate::scraper::anidb::{AnidbClient, AnidbFetch, EpisodeRef, FetchResponse};
use std::sync::Mutex;

/// Languages rows for a listing whose first `dubbed` episodes carry
/// an `eng` embed alongside `jpn`; the rest are sub-only. Records
/// every episode id asked, so a test can pin how much the search
/// had to touch.
struct Dubbed {
    dubbed: u64,
    asked: Mutex<Vec<u64>>,
    /// Ids answered with a not-found status instead of a row.
    dead: &'static [u64],
}

impl Dubbed {
    fn new(dubbed: u64) -> Self {
        Self {
            dubbed,
            asked: Mutex::new(Vec::new()),
            dead: &[],
        }
    }

    fn asked(&self) -> Vec<u64> {
        self.asked.lock().expect("asked").clone()
    }
}

#[async_trait::async_trait]
impl AnidbFetch for &Dubbed {
    async fn get(&self, url: &str) -> crate::error::Result<FetchResponse> {
        let id: u64 = url
            .split("/episode/")
            .nth(1)
            .and_then(|rest| rest.split('/').next())
            .and_then(|s| s.parse().ok())
            .expect("a languages url");
        self.asked.lock().expect("asked").push(id);
        if self.dead.contains(&id) {
            return Ok(FetchResponse {
                status: 404,
                body: String::new(),
            });
        }
        let langs = if id <= self.dubbed {
            r#"[{"code":"jpn","embed_url":"https://e/s"},{"code":"eng","embed_url":"https://e/d"}]"#
        } else {
            r#"[{"code":"jpn","embed_url":"https://e/s"}]"#
        };
        Ok(FetchResponse {
            status: 200,
            body: format!(r#"{{"languages":{langs}}}"#),
        })
    }
}

/// Episode ids 1..=n, numbered 1..=n.
fn listing(n: u64) -> Vec<EpisodeRef> {
    (1..=n)
        .map(|i| EpisodeRef {
            id: i,
            number: u32::try_from(i).expect("small"),
            number2: None,
        })
        .collect()
}

#[tokio::test]
async fn a_present_mode_costs_one_request() {
    // Every sub probe and every dubbed show: the first row answers,
    // and that is the whole question.
    let provider = Dubbed::new(12);
    let client = AnidbClient::new(&provider);
    let eps = listing(12);
    let present = mode_present(&client, &eps, "dub")
        .await
        .expect("the provider answered");
    assert_eq!(present, Some(true));
    assert_eq!(provider.asked(), vec![1], "one row settles it");
}

#[tokio::test]
async fn an_absent_mode_is_an_answered_negative() {
    // Sub-only show asked for dub: the row answers, and the absence
    // is cacheable.
    let provider = Dubbed::new(0);
    let client = AnidbClient::new(&provider);
    let eps = listing(12);
    let present = mode_present(&client, &eps, "dub")
        .await
        .expect("the provider answered");
    assert_eq!(present, Some(false));
    assert_eq!(provider.asked(), vec![1]);
}

#[tokio::test]
async fn a_partly_dubbed_show_has_the_mode() {
    // Two of twelve dubbed. The show carries a dub — that is the
    // question this answers. Which episodes carry it is per-episode
    // data this deliberately does not claim.
    let provider = Dubbed::new(2);
    let client = AnidbClient::new(&provider);
    let eps = listing(12);
    let present = mode_present(&client, &eps, "dub")
        .await
        .expect("the provider answered");
    assert_eq!(present, Some(true));
}

#[tokio::test]
async fn a_dead_row_is_stepped_over_to_a_live_one() {
    // A stale id the provider no longer serves says nothing about
    // the mode; the next row does.
    let provider = Dubbed {
        dubbed: 12,
        asked: Mutex::new(Vec::new()),
        dead: &[1],
    };
    let client = AnidbClient::new(&provider);
    let eps = listing(12);
    let present = mode_present(&client, &eps, "dub")
        .await
        .expect("an answered row is not weather");
    assert_eq!(present, Some(true));
    assert_eq!(provider.asked(), vec![1, 2], "it stepped over exactly one");
}

#[tokio::test]
async fn a_listing_of_dead_rows_yields_no_verdict() {
    // Every row the search touched answered not-found: the provider
    // said nothing about the mode. That is not absence, and
    // availability must not persist it as one.
    let provider = Dubbed {
        dubbed: 0,
        asked: Mutex::new(Vec::new()),
        dead: &[1, 2, 3, 4],
    };
    let client = AnidbClient::new(&provider);
    let eps = listing(4);
    let verdict = mode_present(&client, &eps, "dub")
        .await
        .expect("dead rows are answers, not weather");
    assert_eq!(verdict, None);
}

#[tokio::test]
async fn the_step_over_is_bounded() {
    // A listing that answers not-found everywhere is not worth one
    // request per episode to confirm.
    let dead: &'static [u64] = Box::leak((1..=64u64).collect::<Vec<_>>().into_boxed_slice());
    let provider = Dubbed {
        dubbed: 0,
        asked: Mutex::new(Vec::new()),
        dead,
    };
    let client = AnidbClient::new(&provider);
    let eps = listing(64);
    let verdict = mode_present(&client, &eps, "dub")
        .await
        .expect("dead rows are answers");
    assert_eq!(verdict, None);
    assert!(
        provider.asked().len() <= 8,
        "the search gives up rather than scanning: {:?}",
        provider.asked().len()
    );
}

#[tokio::test]
async fn an_empty_listing_carries_nothing() {
    let provider = Dubbed::new(0);
    let client = AnidbClient::new(&provider);
    let present = mode_present(&client, &[], "sub")
        .await
        .expect("nothing to ask");
    assert_eq!(present, Some(false));
    assert!(provider.asked().is_empty());
}

/// Every languages fetch dies on transport.
struct DeadTransport;

#[async_trait::async_trait]
impl AnidbFetch for DeadTransport {
    async fn get(&self, _url: &str) -> crate::error::Result<FetchResponse> {
        Err(crate::error::AniError::Network)
    }
}

#[tokio::test]
async fn weather_propagates_instead_of_deciding() {
    // A transport death proves nothing about the mode; the caller
    // needs the typed error so it can feed the breaker and persist
    // nothing.
    let client = AnidbClient::new(DeadTransport);
    let eps = listing(4);
    let err = mode_present(&client, &eps, "dub")
        .await
        .expect_err("weather is not a verdict");
    assert!(matches!(err, crate::error::AniError::Network));
}
