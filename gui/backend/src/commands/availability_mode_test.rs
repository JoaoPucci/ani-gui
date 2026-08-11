use super::mode_prefix_len;
use crate::scraper::anidb::{AnidbClient, AnidbFetch, EpisodeRef, FetchResponse};
use std::sync::Mutex;

/// Languages rows for a listing whose first `dubbed` episodes carry
/// an `eng` embed alongside `jpn`; the rest are sub-only. Records
/// every episode id asked, so a test can pin how many rows the
/// bisection had to touch.
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
async fn a_fully_covered_listing_costs_one_request() {
    // Every sub probe and every fully dubbed show: the last row
    // carries the mode, so the whole listing does — no bisection.
    let provider = Dubbed::new(12);
    let client = AnidbClient::new(&provider);
    let eps = listing(12);
    let covered = mode_prefix_len(&client, &eps, "dub")
        .await
        .expect("the provider answered");
    assert_eq!(covered, 12);
    assert_eq!(
        provider.asked(),
        vec![12],
        "only the last row needs asking when it carries the mode"
    );
}

#[tokio::test]
async fn an_absent_mode_costs_two_requests_and_reports_zero() {
    // Sub-only show asked for dub: last says no, first says no, and
    // the answered absence is the cacheable negative.
    let provider = Dubbed::new(0);
    let client = AnidbClient::new(&provider);
    let eps = listing(12);
    let covered = mode_prefix_len(&client, &eps, "dub")
        .await
        .expect("the provider answered");
    assert_eq!(covered, 0);
    assert_eq!(provider.asked(), vec![12, 1]);
}

#[tokio::test]
async fn a_partial_dub_reports_its_prefix_by_bisection() {
    // Seven of twelve dubbed: the boundary is found without asking
    // every row.
    let provider = Dubbed::new(7);
    let client = AnidbClient::new(&provider);
    let eps = listing(12);
    let covered = mode_prefix_len(&client, &eps, "dub")
        .await
        .expect("the provider answered");
    assert_eq!(covered, 7);
    let asked = provider.asked();
    assert!(
        asked.len() < eps.len(),
        "the boundary is bisected, not scanned: {asked:?}"
    );
}

#[tokio::test]
async fn every_boundary_of_a_listing_is_reported_exactly() {
    // The bisection's invariant, exercised across the whole range
    // rather than at one sample point.
    for dubbed in 0..=16u64 {
        let provider = Dubbed::new(dubbed);
        let client = AnidbClient::new(&provider);
        let eps = listing(16);
        let covered = mode_prefix_len(&client, &eps, "dub")
            .await
            .expect("the provider answered");
        assert_eq!(
            covered,
            usize::try_from(dubbed).expect("small"),
            "boundary at {dubbed} misreported"
        );
    }
}

#[tokio::test]
async fn an_empty_listing_covers_nothing() {
    let provider = Dubbed::new(0);
    let client = AnidbClient::new(&provider);
    let covered = mode_prefix_len(&client, &[], "sub")
        .await
        .expect("nothing to ask");
    assert_eq!(covered, 0);
    assert!(provider.asked().is_empty());
}

#[tokio::test]
async fn a_dead_row_does_not_truncate_the_prefix() {
    // A stale episode id the provider no longer serves answers
    // not-found. That is a verdict about the ROW, not about the
    // mode: it must not read as "the dub stops here".
    let provider = Dubbed {
        dubbed: 12,
        asked: Mutex::new(Vec::new()),
        dead: &[12],
    };
    let client = AnidbClient::new(&provider);
    let eps = listing(12);
    let covered = mode_prefix_len(&client, &eps, "dub")
        .await
        .expect("an answered row is not weather");
    assert_eq!(covered, 12);
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
    let err = mode_prefix_len(&client, &eps, "dub")
        .await
        .expect_err("weather is not a verdict");
    assert!(matches!(err, crate::error::AniError::Network));
}
