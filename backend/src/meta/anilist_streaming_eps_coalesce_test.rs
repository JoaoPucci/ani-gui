//! A lane's life: the lookup that opens it goes out at once, the ones
//! arriving while it is out go together a window later, lookups that
//! arrive while a batch is out do not wait for it, a batch every caller
//! left is not sent, lanes of different gate classes never share a
//! request, and a lane whose driver went away opens again.

use std::time::Duration;

use super::*;
use crate::meta::anilist_batch::WINDOW;
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
async fn the_lookup_that_opens_a_lane_goes_out_at_once() {
    // A card's thumbnail is on the path out of its skeleton: the first
    // lookup waits for no window.
    let server = serve(fake(1)).await;
    let base = server.uri();
    let started = tokio::time::Instant::now();
    let got = in_background(streaming_listing_coalesced(
        &reqwest::Client::new(),
        "at-once",
        by_mal(1),
        Some(&base),
    ))
    .await;
    let took = started.elapsed();
    assert_eq!(got.expect("answered"), fake(1).media[0].listing());
    assert!(took < WINDOW * 7 / 10, "the first lookup waited: {took:?}");
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
    let rest = async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        futures_util::future::join_all((2..=5).map(ask)).await
    };
    let (first, rest) = tokio::join!(ask(1), rest);
    assert_eq!(first.expect("answered"), fake(5).media[0].listing());
    for (got, m) in rest.into_iter().zip(&fake(5).media[1..]) {
        assert_eq!(got.expect("answered"), m.listing());
    }
    assert_eq!(
        asked_of(&server).await,
        vec![Asked::ByMal(1), Asked::PageByMal(vec![2, 3, 4, 5])]
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
async fn a_page_that_cannot_be_read_still_answers_each_show_in_time() {
    // AniList nulls a media element of the page, so each gathered show
    // is asked by its single lookup. Those go together, not one after
    // another, so every show is answered inside a caller's budget.
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
        .respond_with(SlowSingles(fake(9), Duration::from_millis(300)))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "unreadable-page";
    let ask = |mal| {
        tokio::time::timeout(
            Duration::from_millis(900),
            in_background(streaming_listing_coalesced(
                &client,
                lane,
                by_mal(mal),
                Some(&base),
            )),
        )
    };
    let ask = &ask;
    let rest = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        futures_util::future::join_all((1..=4).map(ask)).await
    };
    let (first, rest) = tokio::join!(ask(9), rest);
    assert!(first.is_ok_and(|a| a.is_ok()));
    for (got, m) in rest.into_iter().zip(&fake(9).media) {
        let got = got.expect("answered in time");
        assert_eq!(got.expect("answered"), m.listing());
    }
}

#[tokio::test]
async fn a_show_its_page_answered_is_not_held_behind_a_single_lookup() {
    // Two media record MAL id 1, so its show is asked by the single
    // lookup, which is slow. The show the page answered is answered
    // with the page, not after that lookup.
    let mut fake = fake(9);
    fake.media.push(FakeMedia {
        id: 500,
        mal: Some(1),
        listed: 1,
    });
    let server = serve(SlowSingles(fake, Duration::from_millis(600))).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "not-held-behind";
    let started = tokio::time::Instant::now();
    let (client, base) = (&client, base.as_str());
    let ask = |mal| async move {
        let got = in_background(streaming_listing_coalesced(
            client,
            lane,
            by_mal(mal),
            Some(base),
        ))
        .await;
        (got, started.elapsed())
    };
    let ask = &ask;
    let rest = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        tokio::join!(ask(1), ask(2))
    };
    let (_, ((one, _), (two, two_took))) = tokio::join!(ask(9), rest);
    assert!(one.is_ok());
    assert!(two.is_ok());
    assert!(
        two_took < Duration::from_millis(450),
        "the page's show waited for the single lookup: {two_took:?}"
    );
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
    // the AniList-id page, asked for another show, is slow. Show 1's
    // lookup goes as soon as the MAL page is read.
    let mut known = fake(9);
    known.media.push(FakeMedia {
        id: 30,
        mal: None,
        listed: 1,
    });
    let server = serve(SlowIdPage(known, Duration::from_millis(600))).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "single-beside-id-page";
    let started = tokio::time::Instant::now();
    let (client, base) = (&client, base.as_str());
    let ask = |want| async move {
        let got = in_background(streaming_listing_coalesced(client, lane, want, Some(base))).await;
        (got, started.elapsed())
    };
    let ask = &ask;
    let rest = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        tokio::join!(
            ask(by_mal(1)),
            ask(Want {
                mal: None,
                anilist: Some(30),
            })
        )
    };
    let (_, ((one, one_took), (thirty, _))) = tokio::join!(ask(by_mal(9)), rest);
    assert_eq!(one.expect("answered"), fake(9).media[0].listing());
    assert!(thirty.is_ok());
    assert!(
        one_took < Duration::from_millis(450),
        "the single lookup waited for the AniList-id page: {one_took:?}"
    );
}

#[tokio::test]
async fn a_show_the_id_page_answered_is_not_held_behind_the_mal_page_s_single_lookup() {
    // Two media record MAL id 1, so its show goes to a slow single
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
    let server = serve(SlowSingles(known, Duration::from_millis(600))).await;
    let client = reqwest::Client::new();
    let base = server.uri();
    let lane = "id-page-not-held";
    let started = tokio::time::Instant::now();
    let (client, base) = (&client, base.as_str());
    let ask = |want| async move {
        let got = in_background(streaming_listing_coalesced(client, lane, want, Some(base))).await;
        (got, started.elapsed())
    };
    let ask = &ask;
    let rest = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        tokio::join!(
            ask(by_mal(1)),
            ask(Want {
                mal: None,
                anilist: Some(30),
            })
        )
    };
    let (_, ((one, _), (thirty, thirty_took))) = tokio::join!(ask(by_mal(9)), rest);
    assert!(one.is_ok());
    assert_eq!(
        thirty.expect("answered"),
        FakeMedia {
            id: 30,
            mal: None,
            listed: 1
        }
        .listing()
    );
    assert!(
        thirty_took < Duration::from_millis(450),
        "the id page's show waited for the MAL page's single lookup: {thirty_took:?}"
    );
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
    // Slow enough that the first lookup is still out when its runtime
    // goes.
    let (server, base) = server_rt.block_on(async {
        let server = serve(Slow(fake(2), Duration::from_millis(300))).await;
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
