use super::*;

fn at(ms: u64) -> tokio::time::Instant {
    tokio::time::Instant::now() + std::time::Duration::from_millis(ms)
}

#[test]
fn nothing_met_is_no_verdict() {
    assert!(Transients::default().verdict().is_none());
}

#[test]
fn a_transport_failure_met_anywhere_is_the_transient_verdict() {
    for stage in [WalkStage::Search, WalkStage::Pick, WalkStage::Chain] {
        let mut t = Transients::default();
        t.note(stage, AniError::Network, at(1));
        let ne = t.verdict().expect("a failure was met");
        assert!(matches!(ne.error, AniError::Network), "{stage:?}: {ne:?}");
        assert!(!ne.clean_miss, "a transient verdict is never a clean miss");
    }
}

#[test]
fn a_status_an_episode_chain_answered_stands_as_itself() {
    // The breaker reads a non-blocking status as the provider
    // answering; the transport failure substituted for it would read
    // as distress.
    let mut t = Transients::default();
    t.note(WalkStage::Search, AniError::Network, at(1));
    t.note(WalkStage::Chain, AniError::Upstream { status: 401 }, at(2));
    t.note(WalkStage::Search, AniError::Network, at(3));
    let ne = t.verdict().expect("failures were met");
    assert!(
        matches!(ne.error, AniError::Upstream { status: 401 }),
        "{ne:?}"
    );
}

#[test]
fn the_verdict_carries_the_last_failing_attempts_instant() {
    // The breaker record keys its staleness filter on this instant;
    // a recovery recorded between two failing aliases must be able to
    // discard the earlier one, and the later one is what stands.
    let first = at(1);
    let last = at(5);
    let mut t = Transients::default();
    t.note(WalkStage::Chain, AniError::Upstream { status: 401 }, first);
    t.note(WalkStage::Search, AniError::Network, last);
    let ne = t.verdict().expect("failures were met");
    assert_eq!(ne.failed_at, Some(last));
}
