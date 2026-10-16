use super::mode_present;
use crate::error::AniError;
use crate::scraper::provider::EpisodeRef;
use crate::scraper::table_provider::{Answer, Asked, TableProvider};

/// A listing of `n` episodes whose first `dubbed` carry dub as well
/// as sub; the rest are sub-only. Ids in `dead` answer the mode
/// question with a not-found status instead of a row.
fn dubbed(n: u64, dubbed: u64, dead: &[u64]) -> TableProvider {
    (1..=n).fold(TableProvider::new(), |t, id| {
        if dead.contains(&id) {
            let gone = || AniError::Upstream { status: 404 };
            t.mode(id, "sub", Answer::fails(gone))
                .mode(id, "dub", Answer::fails(gone))
        } else {
            t.mode(id, "sub", true).mode(id, "dub", id <= dubbed)
        }
    })
}

/// The episode ids the mode search asked about, in order.
fn asked(provider: &TableProvider) -> Vec<u64> {
    provider
        .asked()
        .into_iter()
        .filter_map(|a| match a {
            Asked::Mode(id, _) => Some(id),
            _ => None,
        })
        .collect()
}

/// Episode ids 1..=n, numbered 1..=n.
fn listing(n: u64) -> Vec<EpisodeRef> {
    (1..=n)
        .map(|i| EpisodeRef {
            id: i,
            number: u32::try_from(i).expect("small"),
            number2: None,
            former_tag: None,
        })
        .collect()
}

#[tokio::test]
async fn a_present_mode_costs_one_request() {
    // Every sub probe and every dubbed show: the first row answers,
    // and that is the whole question.
    let provider = dubbed(12, 12, &[]);
    let eps = listing(12);
    let present = mode_present(&provider, &eps, "dub")
        .await
        .expect("the provider answered");
    assert_eq!(present, Some(true));
    assert_eq!(asked(&provider), vec![1], "one row settles it");
}

#[tokio::test]
async fn an_absent_mode_is_an_answered_negative() {
    // Sub-only show asked for dub: the row answers, and the absence
    // is cacheable.
    let provider = dubbed(12, 0, &[]);
    let eps = listing(12);
    let present = mode_present(&provider, &eps, "dub")
        .await
        .expect("the provider answered");
    assert_eq!(present, Some(false));
    assert_eq!(asked(&provider), vec![1]);
}

#[tokio::test]
async fn a_partly_dubbed_show_has_the_mode() {
    // Two of twelve dubbed. The show carries a dub — that is the
    // question this answers. Which episodes carry it is per-episode
    // data this deliberately does not claim.
    let provider = dubbed(12, 2, &[]);
    let eps = listing(12);
    let present = mode_present(&provider, &eps, "dub")
        .await
        .expect("the provider answered");
    assert_eq!(present, Some(true));
}

#[tokio::test]
async fn a_dead_row_is_stepped_over_to_a_live_one() {
    // A stale id the provider no longer serves says nothing about
    // the mode; the next row does.
    let provider = dubbed(12, 12, &[1]);
    let eps = listing(12);
    let present = mode_present(&provider, &eps, "dub")
        .await
        .expect("an answered row is not weather");
    assert_eq!(present, Some(true));
    assert_eq!(asked(&provider), vec![1, 2], "it stepped over exactly one");
}

#[tokio::test]
async fn a_listing_of_dead_rows_yields_no_verdict() {
    // Every row the search touched answered not-found: the provider
    // said nothing about the mode. That is not absence, and
    // availability must not persist it as one.
    let provider = dubbed(4, 0, &[1, 2, 3, 4]);
    let eps = listing(4);
    let verdict = mode_present(&provider, &eps, "dub")
        .await
        .expect("dead rows are answers, not weather");
    assert_eq!(verdict, None);
}

#[tokio::test]
async fn the_step_over_is_bounded() {
    // A listing that answers not-found everywhere is not worth one
    // request per episode to confirm.
    let dead: Vec<u64> = (1..=64).collect();
    let provider = dubbed(64, 0, &dead);
    let eps = listing(64);
    let verdict = mode_present(&provider, &eps, "dub")
        .await
        .expect("dead rows are answers");
    assert_eq!(verdict, None);
    assert!(
        asked(&provider).len() <= 8,
        "the search gives up rather than scanning: {:?}",
        asked(&provider).len()
    );
}

#[tokio::test]
async fn an_empty_listing_carries_nothing() {
    let provider = dubbed(0, 0, &[]);
    let present = mode_present(&provider, &[], "sub")
        .await
        .expect("nothing to ask");
    assert_eq!(present, Some(false));
    assert!(asked(&provider).is_empty());
}

#[tokio::test]
async fn weather_propagates_instead_of_deciding() {
    // A transport death proves nothing about the mode; the caller
    // needs the typed error so it can feed the breaker and persist
    // nothing.
    let provider = TableProvider::new().modes_default(Answer::fails(|| AniError::Network));
    let eps = listing(4);
    let err = mode_present(&provider, &eps, "dub")
        .await
        .expect_err("weather is not a verdict");
    assert!(matches!(err, AniError::Network));
}
