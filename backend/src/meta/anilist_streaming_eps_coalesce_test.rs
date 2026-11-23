//! A lane's life: the lookup that opens it goes out at once, the ones
//! arriving while it is out go together a window later, lookups that
//! arrive while a batch is out do not wait for it, a batch every caller
//! left is not sent, lanes of different gate classes never share a
//! request, and a lane whose driver went away opens again.

use std::time::Duration;

use super::*;
use crate::meta::anilist_gate::{as_retry, in_background, share_for_tests, Pace, ANILIST};
use crate::meta::anilist_streaming_eps_fake_test::{
    asked_in_any_order, asked_of, sorted, Asked, FakeAniList, FakeMedia,
};

/// How long a held answer is held: past any wait a test makes for
/// another answer ([`IN_TIME`]), so an answer that comes inside that
/// wait did not wait for the held one.
const HELD: Duration = Duration::from_secs(30);

/// How long a test waits for an answer that waits for nothing held:
/// long enough for any machine, short of [`HELD`].
const IN_TIME: Duration = Duration::from_secs(20);

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

/// The stand-in answering once the test lets the request through.
struct HeldFake(crate::held::Hold, FakeAniList);

impl wiremock::Respond for HeldFake {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        self.0.wait_blocking();
        self.1.respond(request)
    }
}

#[tokio::test]
async fn the_lookup_that_opens_a_lane_goes_out_at_once() {
    // A card's thumbnail is on the path out of its skeleton: the first
    // lookup waits for no window. Tokio's clock stands still until the
    // lookup reaches AniList, so no window can pass on its way there: a
    // lookup that waited for one would never arrive.
    let server = wiremock::MockServer::start().await;
    let (hold, gate) = crate::held::gate();
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(HeldFake(hold, fake(1)))
        .mount(&server)
        .await;
    let base = server.uri();
    let client = reqwest::Client::new();
    let reached = async {
        gate.asked().await;
        gate.release();
    };
    let got = crate::held::frozen_until(
        in_background(streaming_listing_coalesced(
            &client,
            "at-once",
            by_mal(1),
            Some(&base),
        )),
        reached,
    )
    .await
    .out;
    assert_eq!(got.expect("answered"), fake(1).media[0].listing());
}

#[tokio::test]
async fn lookups_arriving_while_the_first_is_out_go_together() {
    let server = serve(Slow(fake(5), Duration::from_millis(300))).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "behind-the-first";
    let ask = |mal| {
        in_background(streaming_listing_coalesced(
            &client,
            lane,
            by_mal(mal),
            Some(&base),
        ))
    };
    // The rest arrive in the turn the first opened the lane, its lookup
    // handed to a task of its own but not yet sent, and before the lane's
    // driver can take a window: polled in order, on one thread. The lane
    // tells no difference between a lookup handed off and one sent.
    let rest = futures_util::future::join_all((2..=5).map(ask));
    let (first, rest) = tokio::join!(ask(1), rest);
    assert_eq!(first.expect("answered"), fake(5).media[0].listing());
    for (got, m) in rest.into_iter().zip(&fake(5).media[1..]) {
        assert_eq!(got.expect("answered"), m.listing());
    }
    assert_eq!(
        asked_in_any_order(&server).await,
        sorted(vec![Asked::ByMal(1), Asked::PageByMal(vec![2, 3, 4, 5])])
    );
}

/// The stand-in answering single lookups after `delay` and pages at once.
struct SlowSingles(FakeAniList, Duration);

impl wiremock::Respond for SlowSingles {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let single = matches!(
            Asked::of(&request.body),
            Some(Asked::ByMal(_) | Asked::ById(_))
        );
        let answer = self.0.respond(request);
        if single {
            answer.set_delay(self.1)
        } else {
            answer
        }
    }
}

#[tokio::test]
async fn a_page_that_cannot_be_read_asks_each_show_s_single_lookup_at_once() {
    // AniList nulls a media element of the page, so each gathered show
    // is asked by its single lookup. Those go together, not one after
    // another, so none waits on the others' answers: all four are out
    // at once, each answer held past the test. Which answer reaches
    // which show is `a_page_that_cannot_be_read_hands_its_shows_to_their_single_lookups`'s.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("malIds"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[null]}}}"#,
        ))
        .with_priority(1)
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(SlowSingles(fake(9), HELD))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "unreadable-page";
    let ask = |mal| {
        in_background(streaming_listing_coalesced(
            &client,
            lane,
            by_mal(mal),
            Some(&base),
        ))
    };
    let lookups =
        futures_util::future::join(ask(9), futures_util::future::join_all((1..=4).map(ask)));
    let all_out = async {
        let deadline = tokio::time::Instant::now() + IN_TIME;
        loop {
            let asked = asked_of(&server).await;
            if (1..=4).all(|m| asked.contains(&Asked::ByMal(m))) {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the single lookups went one after another: {asked:?}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    tokio::select! {
        _ = lookups => panic!("answered before the held answers"),
        () = all_out => {}
    }
}

#[tokio::test]
async fn a_show_its_page_answered_is_not_held_behind_a_single_lookup() {
    // Two media record MAL id 1, so its show is asked by the single
    // lookup, which is held. The show the page answered is answered
    // with the page, not after that lookup.
    let mut fake = fake(9);
    fake.media.push(FakeMedia {
        id: 500,
        mal: Some(1),
        listed: 1,
    });
    let server = serve(SlowSingles(fake, HELD)).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "not-held-behind";
    let ask = |mal| {
        in_background(streaming_listing_coalesced(
            &client,
            lane,
            by_mal(mal),
            Some(&base),
        ))
    };
    // Show 9 opens the lane; 1 and 2 gather behind it.
    let mut nine = std::pin::pin!(ask(9));
    let mut one = std::pin::pin!(ask(1));
    assert!(futures_util::poll!(&mut nine).is_pending());
    assert!(futures_util::poll!(&mut one).is_pending());
    let two = tokio::time::timeout(IN_TIME, ask(2))
        .await
        .expect("the page's show waited for the single lookup");
    assert!(two.is_ok());
}

/// The stand-in answering AniList-id pages after `delay`, and a page of
/// MAL ids as one AniList cannot read.
struct SlowIdPage(FakeAniList, Duration);

impl wiremock::Respond for SlowIdPage {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        match Asked::of(&request.body) {
            Some(Asked::PageByMal(_)) => wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[null]}}}"#,
            ),
            Some(Asked::PageById(_)) => self.0.respond(request).set_delay(self.1),
            _ => self.0.respond(request),
        }
    }
}

#[tokio::test]
async fn a_single_lookup_the_mal_page_hands_over_does_not_wait_for_the_id_page() {
    // The MAL page cannot be read, so show 1 goes to its single lookup;
    // the AniList-id page, asked for another show, is held. Show 1's
    // lookup goes as soon as the MAL page is read.
    let mut known = fake(9);
    known.media.push(FakeMedia {
        id: 30,
        mal: None,
        listed: 1,
    });
    let server = serve(SlowIdPage(known, HELD)).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "single-beside-id-page";
    let ask = |want| {
        in_background(streaming_listing_coalesced(
            &client,
            lane,
            want,
            Some(&base),
        ))
    };
    // Show 9 opens the lane; 1 and the show mapped to AniList alone
    // gather behind it.
    let mut nine = std::pin::pin!(ask(by_mal(9)));
    let mut thirty = std::pin::pin!(ask(Want {
        mal: None,
        anilist: Some(30),
    }));
    assert!(futures_util::poll!(&mut nine).is_pending());
    assert!(futures_util::poll!(&mut thirty).is_pending());
    let one = tokio::time::timeout(IN_TIME, ask(by_mal(1)))
        .await
        .expect("the single lookup waited for the AniList-id page");
    assert_eq!(one.expect("answered"), fake(9).media[0].listing());
}

#[tokio::test]
async fn a_show_the_id_page_answered_is_not_held_behind_the_mal_page_s_single_lookup() {
    // Two media record MAL id 1, so its show goes to a held single
    // lookup; the show Kitsu maps to AniList alone is answered by the
    // AniList-id page, and gets that answer as the page comes in.
    let mut known = fake(9);
    known.media.push(FakeMedia {
        id: 500,
        mal: Some(1),
        listed: 1,
    });
    known.media.push(FakeMedia {
        id: 30,
        mal: None,
        listed: 1,
    });
    let server = serve(SlowSingles(known, HELD)).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "id-page-not-held";
    let ask = |want| {
        in_background(streaming_listing_coalesced(
            &client,
            lane,
            want,
            Some(&base),
        ))
    };
    // Show 9 opens the lane; 1 and the show mapped to AniList alone
    // gather behind it.
    let mut nine = std::pin::pin!(ask(by_mal(9)));
    let mut one = std::pin::pin!(ask(by_mal(1)));
    assert!(futures_util::poll!(&mut nine).is_pending());
    assert!(futures_util::poll!(&mut one).is_pending());
    let thirty = tokio::time::timeout(
        IN_TIME,
        ask(Want {
            mal: None,
            anilist: Some(30),
        }),
    )
    .await
    .expect("the id page's show waited for the MAL page's single lookup");
    assert_eq!(
        thirty.expect("answered"),
        FakeMedia {
            id: 30,
            mal: None,
            listed: 1
        }
        .listing()
    );
}

/// The stand-in holding the single lookup of MAL id 1's answer.
struct HoldsFirst(FakeAniList);

impl wiremock::Respond for HoldsFirst {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let answer = self.0.respond(request);
        if Asked::of(&request.body) == Some(Asked::ByMal(1)) {
            answer.set_delay(HELD)
        } else {
            answer
        }
    }
}

#[tokio::test]
async fn a_lookup_arriving_while_a_batch_is_out_does_not_wait_for_it() {
    // A's request is out and its answer held. B, arriving then, goes out
    // on its own rather than after A's answer.
    let server = serve(HoldsFirst(fake(2))).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "late-joiner";
    let ask = |mal| {
        in_background(streaming_listing_coalesced(
            &client,
            lane,
            by_mal(mal),
            Some(&base),
        ))
    };
    // A stays asked for throughout: its caller still waits while B asks.
    let mut a = std::pin::pin!(ask(1));
    let a_out = async {
        let deadline = tokio::time::Instant::now() + IN_TIME;
        while asked_of(&server).await.is_empty() {
            assert!(tokio::time::Instant::now() < deadline, "A never went out");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    tokio::select! {
        _ = &mut a => panic!("A was answered"),
        () = a_out => {}
    }
    let b = tokio::time::timeout(IN_TIME, ask(2))
        .await
        .expect("B waited for A's batch");
    assert_eq!(b.expect("answered"), fake(2).media[1].listing());
    assert!(futures_util::poll!(&mut a).is_pending(), "A was still out");
    assert_eq!(
        asked_of(&server).await,
        vec![Asked::ByMal(1), Asked::ByMal(2)]
    );
}

#[tokio::test]
async fn a_batch_every_caller_left_is_not_sent() {
    // The gate keeps half a minute between requests. The first lookup
    // takes the turn; the next two queue behind it in the gate, and
    // their callers stop waiting there: the batch is dropped rather
    // than sent for nobody.
    let server = serve(fake(3)).await;
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
    let lane = "all-left";
    let ask = |mal| {
        in_background(streaming_listing_coalesced(
            &client,
            lane,
            by_mal(mal),
            Some(&base),
        ))
    };
    assert!(ask(1).await.is_ok());
    let queued = crate::held::until("the batch waits for its turn", || gate.waiting() == 1);
    // Gathered, so the two go as one batch whether or not the first's
    // lane is still open.
    tokio::select! {
        _ = crate::meta::anilist_batch::gathered(futures_util::future::join(ask(2), ask(3))) => {
            panic!("answered inside the gap")
        }
        () = queued => {}
    }
    // Its callers gone, the batch leaves the gate's queue unsent.
    crate::held::until("the batch leaves the queue", || gate.waiting() == 0).await;
    assert_eq!(
        gate.let_go(),
        vec![crate::meta::anilist_gate::Class::Background]
    );
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
    // The first lookup's answer is held, so it is still out when its
    // runtime goes.
    let (hold, gate) = crate::held::gate();
    let (server, base) = server_rt.block_on(async {
        let server = serve(HeldFake(hold, fake(2))).await;
        let base = server.uri();
        (server, base)
    });
    // Declared after the server, so a failure releases the held answer
    // before the server's drop waits on it.
    let gate = gate;
    let lane = "runtime-gone";
    let first = runtime();
    let client = reqwest::Client::new();
    let (still, standing) = std::sync::mpsc::channel::<()>();
    let out = first.block_on(async {
        // The clock stands still until the request is out, so the
        // runtime goes while its driver is mid-window however long the
        // request takes to arrive: a blocking task keeps a paused clock
        // from moving on.
        tokio::time::pause();
        tokio::task::spawn_blocking(move || {
            let _ = standing.recv();
        });
        tokio::select! {
            _ = in_background(streaming_listing_coalesced(
                &client,
                lane,
                by_mal(1),
                Some(&base),
            )) => false,
            () = gate.asked() => true,
        }
    });
    assert_eq!(
        <super::Streaming as crate::meta::anilist_batch::Kind>::lanes().opened(lane),
        1,
        "the lane was still open when its runtime went"
    );
    drop(still);
    assert!(out, "the first lookup was answered");
    drop(first);
    gate.release();
    // Bounded, so a lane left open fails the test instead of hanging it.
    let answered = runtime().block_on(async {
        tokio::time::timeout(
            Duration::from_secs(5),
            in_background(streaming_listing_coalesced(
                &reqwest::Client::new(),
                lane,
                by_mal(2),
                Some(&base),
            )),
        )
        .await
    });
    assert_eq!(
        answered.expect("answered in time").expect("answered"),
        fake(2).media[1].listing()
    );
    server_rt.block_on(async move { drop(server) });
}
