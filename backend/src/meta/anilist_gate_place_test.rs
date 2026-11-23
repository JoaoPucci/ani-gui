//! A request waits for its turn only within its class's wait; a place a
//! lane holds counts as a request waiting, and is given up as the first
//! request handed it takes its turn.

use std::time::Duration;

use proptest::prelude::*;

use super::*;
use crate::meta::anilist_gate::{share_for_tests, Ask, Pace, Purpose, ANILIST};

const ASK: Ask = Ask::unkeyed(Purpose::Trending);

proptest! {
    #[test]
    fn a_wait_is_kept_only_inside_the_budget_and_unchanged(wait in 0u64..10_000, budget in 0u64..10_000) {
        let (wait, budget) = (Duration::from_millis(wait), Duration::from_millis(budget));
        match within(wait, budget) {
            Some(kept) => {
                prop_assert_eq!(kept, wait);
                prop_assert!(kept <= budget);
            }
            None => prop_assert!(wait > budget),
        }
    }
}

#[test]
fn a_place_counts_as_waiting_until_it_is_dropped() {
    let url = "http://gate-place.invalid";
    let gate = share_for_tests(url, ANILIST);
    let held = place(url);
    assert_eq!(gate.waiting(), 1);
    drop(held);
    assert_eq!(gate.waiting(), 0);
}

#[tokio::test]
async fn a_handed_place_is_given_up_as_the_request_takes_its_turn() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;
    let uri = server.uri();
    let gate = share_for_tests(&uri, ANILIST);
    let watched = &gate;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let sent = handing(Some(place(&uri)), deadline, async {
        assert_eq!(watched.waiting(), 1, "held while it is handed");
        let got = crate::meta::anilist::post_graphql_public(
            &reqwest::Client::new(),
            &uri,
            &serde_json::json!({}),
            ASK,
        )
        .await;
        assert_eq!(watched.waiting(), 0, "given up with the turn");
        got
    })
    .await;
    assert!(sent.is_ok());
}

#[tokio::test]
async fn a_turn_comes_in_the_gap_and_past_the_wait_is_none() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;
    let uri = server.uri();
    let _gate = share_for_tests(
        &uri,
        Pace {
            min_gap: Duration::from_secs(30),
            ..ANILIST
        },
    );
    assert_eq!(turn_in(&uri, Class::Background), Some(Duration::ZERO));
    let _ = crate::meta::anilist_gate::in_background(crate::meta::anilist::post_graphql_public(
        &reqwest::Client::new(),
        &uri,
        &serde_json::json!({}),
        ASK,
    ))
    .await;
    // Thirty seconds to the next turn: past a background fill's wait.
    assert_eq!(turn_in(&uri, Class::Background), None);
}

#[tokio::test]
async fn a_request_handed_a_deadline_its_turn_comes_past_is_turned_away_at_once() {
    // The lookups a lane handed over already spent their wait there: a
    // turn past what is left of it is not waited for, however long the
    // class's own wait.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;
    let uri = server.uri();
    let _gate = share_for_tests(
        &uri,
        Pace {
            min_gap: Duration::from_secs(30),
            background_wait: Duration::from_secs(60),
            ..ANILIST
        },
    );
    let client = reqwest::Client::new();
    let body = serde_json::json!({});
    let post = || {
        crate::meta::anilist_gate::in_background(crate::meta::anilist::post_graphql_public(
            &client, &uri, &body, ASK,
        ))
    };
    assert!(post().await.is_ok());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    let got = tokio::time::timeout(Duration::from_secs(10), handing(None, deadline, post()))
        .await
        .expect("turned away at once, not after a wait");
    assert!(
        matches!(got, Err(crate::error::AniError::RateLimited { .. })),
        "{got:?}"
    );
}
