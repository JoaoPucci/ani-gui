use super::*;

fn at(ms: u64) -> tokio::time::Instant {
    tokio::time::Instant::now() + std::time::Duration::from_millis(ms)
}

fn stream_server() -> AniError {
    AniError::StreamServerFailed {
        stage: "embed",
        host: Some("zokoanime.video".into()),
    }
}

fn unreadable() -> AniError {
    AniError::ParseFailed {
        detail: "the page is not the search shape".into(),
    }
}

/// The error a run of notes ends in, as the walk would see it.
fn verdict_of(notes: Vec<(WalkStage, AniError)>) -> NativeError {
    let mut t = Transients::default();
    for (i, (stage, error)) in notes.into_iter().enumerate() {
        t.note(stage, error, at(i as u64));
    }
    t.verdict().expect("a failure was met")
}

fn shown(e: &AniError) -> String {
    format!("{e:?}")
}

#[test]
fn nothing_met_is_no_verdict() {
    assert!(Transients::default().verdict().is_none());
}

/// A failure met alone is the verdict under its own name, whatever
/// the stage: the walk no longer substitutes a transport failure for
/// what it met, so a search page the client could not read says so,
/// and the stream server that did not serve says so.
#[test]
fn a_failure_met_alone_is_the_verdict_under_its_own_name() {
    for stage in [WalkStage::Search, WalkStage::Pick, WalkStage::Chain] {
        for error in [
            AniError::Network,
            AniError::Timeout,
            unreadable(),
            AniError::Upstream { status: 401 },
            stream_server(),
        ] {
            let want = shown(&error);
            let ne = verdict_of(vec![(stage, error)]);
            assert_eq!(shown(&ne.error), want, "{stage:?}");
            assert!(!ne.clean_miss, "a transient verdict is never a clean miss");
        }
    }
}

/// The failure from further along the walk is the verdict: a chain
/// that did not serve says the show was found, which a search that
/// died on another alias cannot unsay — and the connection the search
/// blames is the one the other alias's search got through on.
#[test]
fn a_chain_failure_outranks_a_search_failure_in_either_order() {
    let want = shown(&stream_server());
    let search_first = verdict_of(vec![
        (WalkStage::Search, AniError::Network),
        (WalkStage::Chain, stream_server()),
    ]);
    assert_eq!(shown(&search_first.error), want);
    let chain_first = verdict_of(vec![
        (WalkStage::Chain, stream_server()),
        (WalkStage::Search, AniError::Network),
    ]);
    assert_eq!(shown(&chain_first.error), want);
}

#[test]
fn a_pick_failure_outranks_a_search_failure_and_yields_to_a_chain_failure() {
    let over_search = verdict_of(vec![
        (WalkStage::Pick, AniError::Timeout),
        (WalkStage::Search, unreadable()),
    ]);
    assert_eq!(shown(&over_search.error), shown(&AniError::Timeout));
    let under_chain = verdict_of(vec![
        (WalkStage::Pick, unreadable()),
        (WalkStage::Chain, stream_server()),
    ]);
    assert_eq!(shown(&under_chain.error), shown(&stream_server()));
}

/// Within the chain, a status a host answered outranks the stream
/// server's failure: the breaker reads the status as the provider
/// answering, where the failure reads as distress.
#[test]
fn within_the_chain_an_answered_status_outranks_the_stream_servers_failure() {
    let want = shown(&AniError::Upstream { status: 401 });
    let status_first = verdict_of(vec![
        (WalkStage::Chain, AniError::Upstream { status: 401 }),
        (WalkStage::Chain, stream_server()),
    ]);
    assert_eq!(shown(&status_first.error), want);
    let status_last = verdict_of(vec![
        (WalkStage::Chain, stream_server()),
        (WalkStage::Chain, AniError::Upstream { status: 401 }),
    ]);
    assert_eq!(shown(&status_last.error), want);
}

/// Within a search, a page the client could not read outranks a
/// connection that dropped: the site answered, so the connection is
/// not what to tell the user about.
#[test]
fn within_a_search_an_unreadable_answer_outranks_a_transport_failure() {
    let want = shown(&unreadable());
    let dropped_first = verdict_of(vec![
        (WalkStage::Search, AniError::Network),
        (WalkStage::Search, unreadable()),
    ]);
    assert_eq!(shown(&dropped_first.error), want);
    let dropped_last = verdict_of(vec![
        (WalkStage::Search, unreadable()),
        (WalkStage::Search, AniError::Timeout),
    ]);
    assert_eq!(shown(&dropped_last.error), want);
}

#[test]
fn of_two_failures_of_one_rank_the_first_stays() {
    let ne = verdict_of(vec![
        (WalkStage::Search, AniError::Network),
        (WalkStage::Search, AniError::Timeout),
    ]);
    assert_eq!(shown(&ne.error), shown(&AniError::Network));
    let ne = verdict_of(vec![
        (WalkStage::Chain, AniError::Upstream { status: 401 }),
        (WalkStage::Chain, AniError::Upstream { status: 400 }),
    ]);
    assert_eq!(shown(&ne.error), shown(&AniError::Upstream { status: 401 }));
}

#[test]
fn the_verdict_carries_the_last_failing_attempts_instant() {
    // The breaker record keys its staleness filter on this instant;
    // a recovery recorded between two failing aliases must be able to
    // discard the earlier one, and the later one is what stands —
    // whichever failure is surfaced.
    let first = at(1);
    let last = at(5);
    let mut t = Transients::default();
    t.note(WalkStage::Chain, AniError::Upstream { status: 401 }, first);
    t.note(WalkStage::Search, AniError::Network, last);
    let ne = t.verdict().expect("failures were met");
    assert_eq!(ne.failed_at, Some(last));
}

mod props {
    use super::*;
    use proptest::prelude::*;

    fn arb_stage() -> impl Strategy<Value = WalkStage> {
        prop_oneof![
            Just(WalkStage::Search),
            Just(WalkStage::Pick),
            Just(WalkStage::Chain),
        ]
    }

    /// The errors a walk notes as transient: transport failures, a
    /// timeout, a page the client could not read, a status a chain
    /// answered that is neither a block nor absence, and the stream
    /// server's failure.
    fn arb_error() -> impl Strategy<Value = AniError> {
        prop_oneof![
            any::<()>().prop_map(|()| AniError::Network),
            any::<()>().prop_map(|()| AniError::Timeout),
            "[a-z ]{0,12}".prop_map(|detail| AniError::ParseFailed { detail }),
            prop_oneof![Just(400u16), Just(401), Just(405), Just(418)]
                .prop_map(|status| AniError::Upstream { status }),
            prop::option::of("[a-z.]{1,12}").prop_map(|host| AniError::StreamServerFailed {
                stage: "embed",
                host,
            }),
        ]
    }

    /// The rule, stated apart from the code: the later stage wins;
    /// within a stage an answered status outranks an unreadable
    /// answer, which outranks the rest; of equals the first stays.
    fn rank(e: &AniError) -> u8 {
        match e {
            AniError::Upstream { .. } => 2,
            AniError::ParseFailed { .. } => 1,
            _ => 0,
        }
    }

    proptest! {
        #[test]
        fn the_verdict_is_the_furthest_loudest_failure_met_first(
            notes in prop::collection::vec((arb_stage(), arb_error()), 1..6),
        ) {
            let expected = notes
                .iter()
                .fold(None::<&(WalkStage, AniError)>, |kept, next| match kept {
                    Some(k) if (k.0, rank(&k.1)) >= (next.0, rank(&next.1)) => Some(k),
                    _ => Some(next),
                })
                .map(|(_, e)| shown(e))
                .expect("at least one note");
            let count = notes.len();
            let mut t = Transients::default();
            let mut last = None;
            for (i, (stage, error)) in notes.into_iter().enumerate() {
                let when = at(i as u64);
                last = Some(when);
                t.note(stage, error, when);
            }
            let ne = t.verdict().expect("notes were made");
            prop_assert_eq!(shown(&ne.error), expected);
            prop_assert!(!ne.clean_miss);
            prop_assert_eq!(ne.failed_at, last, "{} notes", count);
        }
    }
}
