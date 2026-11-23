//! A lane's life: lookups that arrive while a batch is out do not wait
//! for it, a batch every caller left is not sent, lanes of different
//! gate classes never share a request, and a lane whose driver went
//! away opens again.

use std::time::Duration;

use super::*;
use crate::meta::anilist_gate::{as_retry, in_background, share_for_tests, Pace, ANILIST};
use crate::meta::anilist_streaming_eps_fake_test::{asked_of, Asked, FakeAniList, FakeMedia};

fn fake(n: u32) -> FakeAniList {
    FakeAniList {
        media: (1..=n)
            .map(|i| FakeMedia {
                id: 100 + i,
                mal: Some(i),
                listed: 1,
            })
            .collect(),
    }
}

fn by_mal(mal: u32) -> Want {
    Want {
        mal: Some(mal),
        anilist: None,
    }
}

/// The stand-in answering after `delay`.
struct Slow(FakeAniList, Duration);

impl wiremock::Respond for Slow {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        self.0.respond(request).set_delay(self.1)
    }
}

async fn serve(respond: impl wiremock::Respond + 'static) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(respond)
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn a_lookup_arriving_while_a_batch_is_out_does_not_wait_for_it() {
    // A's batch takes 600 ms to answer. B, arriving 200 ms in, goes out
    // in a batch of its own a window later rather than after A's answer.
    let server = serve(Slow(fake(2), Duration::from_millis(600))).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "late-joiner";
    let started = tokio::time::Instant::now();
    let a = in_background(streaming_listing_coalesced(
        &client,
        lane,
        by_mal(1),
        Some(&base),
    ));
    let b = async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let got = in_background(streaming_listing_coalesced(
            &client,
            lane,
            by_mal(2),
            Some(&base),
        ))
        .await;
        (got, started.elapsed())
    };
    let (a, (b, b_took)) = tokio::join!(a, b);
    assert_eq!(a.expect("answered"), fake(2).media[0].listing());
    assert_eq!(b.expect("answered"), fake(2).media[1].listing());
    assert!(
        b_took < Duration::from_millis(1200),
        "B waited for A's batch: {b_took:?}"
    );
    assert_eq!(
        asked_of(&server).await,
        vec![Asked::ByMal(1), Asked::ByMal(2)]
    );
}

#[tokio::test]
async fn a_batch_every_caller_left_is_not_sent() {
    // The gate keeps two seconds between requests. The first lookup
    // takes the turn; the next two queue behind it in the gate, and
    // their callers stop waiting before their turn comes: the batch is
    // dropped rather than sent for nobody.
    let server = serve(fake(3)).await;
    let base = server.uri();
    let _gate = share_for_tests(
        &base,
        Pace {
            min_gap: Duration::from_secs(2),
            background_wait: Duration::from_secs(5),
            ..ANILIST
        },
    );
    let client = reqwest::Client::new();
    let lane = "all-left";
    let first = in_background(streaming_listing_coalesced(
        &client,
        lane,
        by_mal(1),
        Some(&base),
    ))
    .await;
    assert!(first.is_ok());
    let left = |mal| {
        tokio::time::timeout(
            Duration::from_millis(400),
            in_background(streaming_listing_coalesced(
                &client,
                lane,
                by_mal(mal),
                Some(&base),
            )),
        )
    };
    let (b, c) = tokio::join!(left(2), left(3));
    assert!(b.is_err() && c.is_err(), "the callers stopped waiting");
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(asked_of(&server).await, vec![Asked::ByMal(1)]);
}

#[tokio::test]
async fn a_retry_and_a_background_fill_never_share_a_request() {
    let server = serve(fake(2)).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "classes";
    let (a, b) = tokio::join!(
        in_background(streaming_listing_coalesced(
            &client,
            lane,
            by_mal(1),
            Some(&base)
        )),
        as_retry(streaming_listing_coalesced(
            &client,
            lane,
            by_mal(2),
            Some(&base)
        )),
    );
    assert!(a.is_ok() && b.is_ok());
    let mut asked = asked_of(&server).await;
    asked.sort_by_key(|a| format!("{a:?}"));
    assert_eq!(asked, vec![Asked::ByMal(1), Asked::ByMal(2)]);
}

#[test]
fn a_lane_whose_runtime_went_away_opens_again() {
    // A lookup left in a lane when its runtime shuts down — its driver
    // dropped mid-window — does not leave the lane open for good: the
    // next runtime's lookup opens it again and is answered.
    let runtime = || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime")
    };
    let server_rt = runtime();
    let (server, base) = server_rt.block_on(async {
        let server = serve(fake(2)).await;
        let base = server.uri();
        (server, base)
    });
    let lane = "runtime-gone";
    let first = runtime();
    let client = reqwest::Client::new();
    let gone = first.block_on(async {
        tokio::time::timeout(
            Duration::from_millis(20),
            in_background(streaming_listing_coalesced(
                &client,
                lane,
                by_mal(1),
                Some(&base),
            )),
        )
        .await
    });
    assert!(gone.is_err());
    drop(first);
    let answered = runtime().block_on(in_background(streaming_listing_coalesced(
        &reqwest::Client::new(),
        lane,
        by_mal(2),
        Some(&base),
    )));
    assert_eq!(answered.expect("answered"), fake(2).media[1].listing());
    server_rt.block_on(async move { drop(server) });
}
