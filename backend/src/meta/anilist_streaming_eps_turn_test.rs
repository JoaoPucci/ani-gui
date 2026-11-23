//! A lane against the gate it sends through: a lookup the gate would
//! hold back for a turn anyway waits for that turn in its lane, where
//! the lookups after it join it, rather than alone in the gate; one the
//! gate would let go now still goes at once; and the lane's wait counts
//! as a request waiting for its turn, so no retry takes that turn ahead
//! of it, until its callers leave.
//!
//! Each test shares one gate for its mock AniList, paced as AniList's
//! own, and runs on a paused clock from the first lookup on: the clock
//! moves only from timer to timer, so the lookups arrive at the times
//! the test gives them however slow the machine.

use std::time::Duration;

use super::*;
use crate::meta::anilist_gate::{as_retry, in_background, share_for_tests, Class, Pace, ANILIST};
use crate::meta::anilist_streaming_eps_fake_test::{
    asked_in_any_order, asked_of, sorted, Asked, FakeAniList, FakeMedia,
};

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

async fn serve(respond: impl wiremock::Respond + 'static) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(respond)
        .mount(&server)
        .await;
    server
}

/// `mal`'s lookup in `lane`, as a card's fill asks it, `after` the
/// clock's start.
async fn card_at(
    client: &reqwest::Client,
    lane: &str,
    base: &str,
    mal: u32,
    after: Duration,
) -> crate::error::Result<StreamingListing> {
    // A sleep of nothing still waits for the clock's next tick, which a
    // clock that stands still never reaches.
    if !after.is_zero() {
        tokio::time::sleep(after).await;
    }
    in_background(streaming_listing_coalesced(
        client,
        lane,
        by_mal(mal),
        Some(base),
    ))
    .await
}

#[tokio::test]
async fn singles_the_gate_would_hold_behind_the_one_out_share_the_next_page() {
    // Three cards outside a strip's warm ask for their stills 101 ms and
    // 216 ms apart, as a cold Home's did. The first goes at once. The
    // gate keeps a quarter second after it, so the second would wait for
    // its turn anyway: it waits in the lane, the third joins it, and the
    // two go as one page when the turn comes.
    let server = serve(fake(3)).await;
    let base = server.uri();
    let gate = share_for_tests(&base, ANILIST);
    let client = reqwest::Client::new();
    let lane = "staggered-singles";
    tokio::time::pause();
    let (a, b, c) = tokio::join!(
        card_at(&client, lane, &base, 1, Duration::ZERO),
        card_at(&client, lane, &base, 2, Duration::from_millis(101)),
        card_at(&client, lane, &base, 3, Duration::from_millis(216)),
    );
    tokio::time::resume();
    for (got, media) in [a, b, c].into_iter().zip(&fake(3).media) {
        assert_eq!(got.expect("answered"), media.listing());
    }
    assert_eq!(
        asked_in_any_order(&server).await,
        sorted(vec![Asked::ByMal(1), Asked::PageByMal(vec![2, 3])])
    );
    assert_eq!(gate.let_go().len(), 2);
}

#[tokio::test]
async fn a_lookup_the_gate_would_let_go_now_goes_alone_at_once() {
    // Past the gate's quarter second after the last request, a card's
    // lookup is not held for anything: on a clock that stands still
    // until it reaches AniList, it gets there.
    let server = wiremock::MockServer::start().await;
    let (hold, held) = crate::held::gate();
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(HoldsSecond(hold, fake(2)))
        .mount(&server)
        .await;
    let base = server.uri();
    let _gate = share_for_tests(&base, ANILIST);
    let client = reqwest::Client::new();
    let lane = "gate-ready";
    let first = card_at(&client, lane, &base, 1, Duration::ZERO).await;
    assert!(first.is_ok());
    // Past the gap: a sleep is never shorter than it was asked to be.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let reached = async {
        held.asked().await;
        held.release();
    };
    let second =
        crate::held::frozen_until(card_at(&client, lane, &base, 2, Duration::ZERO), reached)
            .await
            .out;
    assert_eq!(second.expect("answered"), fake(2).media[1].listing());
    assert_eq!(
        asked_of(&server).await,
        vec![Asked::ByMal(1), Asked::ByMal(2)]
    );
}

/// The stand-in holding its second request until the test lets it
/// through.
struct HoldsSecond(crate::held::Hold, FakeAniList);

impl wiremock::Respond for HoldsSecond {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        if Asked::of(&request.body) == Some(Asked::ByMal(2)) {
            self.0.wait_blocking();
        }
        self.1.respond(request)
    }
}

#[tokio::test]
async fn a_retry_does_not_take_the_turn_a_lane_waits_for() {
    // A card's lookup waits in its lane for the gate's turn while a
    // retry of a held-back fill waits in the gate for one too. The
    // card's lookup goes first, as it would have from the gate's own
    // queue.
    let server = serve(fake(3)).await;
    let base = server.uri();
    let gate = share_for_tests(&base, ANILIST);
    let client = reqwest::Client::new();
    let lane = "retry-behind-lane";
    let retry_lane = "retry-behind-lane-retry";
    tokio::time::pause();
    let retry = async {
        tokio::time::sleep(Duration::from_millis(150)).await;
        as_retry(streaming_listing_coalesced(
            &client,
            retry_lane,
            by_mal(3),
            Some(&base),
        ))
        .await
    };
    let (a, b, r) = tokio::join!(
        card_at(&client, lane, &base, 1, Duration::ZERO),
        card_at(&client, lane, &base, 2, Duration::from_millis(101)),
        retry,
    );
    tokio::time::resume();
    assert!(a.is_ok() && b.is_ok() && r.is_ok());
    assert_eq!(
        gate.let_go(),
        vec![Class::Background, Class::Background, Class::Retry]
    );
}

#[tokio::test]
async fn callers_that_leave_a_lane_waiting_for_its_turn_leave_the_gate_free() {
    // The gate keeps half a minute between requests. A card's lookup
    // that arrives behind the first waits for that turn; its caller
    // stops waiting, and nothing is left waiting at the gate or sent.
    let server = serve(fake(2)).await;
    let base = server.uri();
    let gate = share_for_tests(
        &base,
        Pace {
            min_gap: Duration::from_secs(30),
            background_wait: Duration::from_secs(60),
            ..ANILIST
        },
    );
    let client = reqwest::Client::new();
    let lane = "left-the-lane";
    assert!(card_at(&client, lane, &base, 1, Duration::ZERO)
        .await
        .is_ok());
    let waiting = crate::held::until("the card's lookup waits for its turn", || {
        gate.waiting() == 1
    });
    tokio::select! {
        _ = card_at(&client, lane, &base, 2, Duration::ZERO) => panic!("answered inside the gap"),
        () = waiting => {}
    }
    crate::held::until("nothing waits at the gate", || gate.waiting() == 0).await;
    assert_eq!(gate.let_go(), vec![Class::Background]);
    assert_eq!(asked_of(&server).await, vec![Asked::ByMal(1)]);
}
