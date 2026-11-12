use std::sync::atomic::{AtomicUsize, Ordering};

use super::{gaps, run, FlagCase, Gaps, CASES};
use crate::flags::decl::FlagValue;
use crate::flags::resolve::Resolved;
use crate::flags::test_decls::{A, B, TEST_DECLS};
use crate::flags::DECLARED;

/// The rule itself: every flag this build declares is tested at every
/// value, and every case runs.
#[test]
fn every_declared_flag_is_tested_at_its_default_and_every_other_value() {
    assert_eq!(gaps(DECLARED, CASES), Gaps::default());
    run(DECLARED, CASES);
}

static RUNS: AtomicUsize = AtomicUsize::new(0);

fn is(flag: usize, on: bool, r: &Resolved) {
    assert_eq!(r.enabled(&TEST_DECLS[flag]), on);
    RUNS.fetch_add(1, Ordering::SeqCst);
}
fn a_off(r: &Resolved) {
    is(0, false, r);
}
fn a_on(r: &Resolved) {
    is(0, true, r);
}
fn b_off(r: &Resolved) {
    is(1, false, r);
}
fn b_on(r: &Resolved) {
    is(1, true, r);
}

const fn case(flag: usize, value: FlagValue, check: fn(&Resolved)) -> FlagCase {
    FlagCase {
        flag: TEST_DECLS[flag],
        value,
        check,
    }
}

const OFF: FlagValue = FlagValue::Bool(false);
const ON: FlagValue = FlagValue::Bool(true);

const FULL: &[FlagCase] = &[
    case(0, OFF, a_off),
    case(0, ON, a_on),
    case(1, OFF, b_off),
    case(1, ON, b_on),
];

#[test]
fn a_full_set_has_no_gaps_and_every_case_runs_with_its_value_in_force() {
    assert_eq!(gaps(TEST_DECLS, FULL), Gaps::default());
    let before = RUNS.load(Ordering::SeqCst);
    run(TEST_DECLS, FULL);
    assert_eq!(RUNS.load(Ordering::SeqCst) - before, FULL.len());
}

#[test]
fn a_flag_tested_only_at_its_default_is_reported_at_its_other_value() {
    let only_defaults = [case(0, OFF, a_off), case(1, OFF, b_off)];
    assert_eq!(
        gaps(TEST_DECLS, &only_defaults).untested,
        vec![(A, ON), (B, ON)]
    );
}

#[test]
fn a_flag_tested_only_switched_on_is_reported_at_its_default() {
    let only_on = [case(0, ON, a_on)];
    let g = gaps(&TEST_DECLS[..1], &only_on);
    assert_eq!(g.untested, vec![(A, OFF)]);
}

#[test]
fn no_cases_at_all_report_every_value() {
    assert_eq!(gaps(TEST_DECLS, &[]).untested.len(), 4);
}

#[test]
fn a_case_for_an_undeclared_flag_is_stray() {
    let stale = [case(0, OFF, a_off), case(1, ON, b_on)];
    let g = gaps(&TEST_DECLS[..1], &stale);
    assert_eq!(g.stray, vec![(B, ON)]);
    assert_eq!(gaps(TEST_DECLS, &stale).stray, vec![]);
}

#[test]
#[should_panic(expected = "assertion")]
fn a_case_whose_check_fails_fails_the_run() {
    // A check that asserts the wrong behaviour for its value.
    let wrong = [case(0, ON, a_off)];
    run(TEST_DECLS, &wrong);
}
