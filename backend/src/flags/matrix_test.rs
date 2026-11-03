use std::sync::atomic::{AtomicUsize, Ordering};

use super::{gaps, run, FlagCase, Gaps, CASES};
use crate::flags::decl::FlagValue;
use crate::flags::resolve::Resolved;
use crate::flags::test_decls::{FALLBACK, KILL, TEST_DECLS};
use crate::flags::DECLARED;

/// The rule itself: every flag this build declares is tested at every
/// value, and every case runs.
#[test]
fn every_declared_flag_is_tested_at_its_default_and_every_retreat() {
    assert_eq!(gaps(DECLARED, CASES), Gaps::default());
    run(DECLARED, CASES);
}

static RUNS: AtomicUsize = AtomicUsize::new(0);

fn kill_on(r: &Resolved) {
    assert!(r.enabled(&TEST_DECLS[0]));
    RUNS.fetch_add(1, Ordering::SeqCst);
}
fn kill_off(r: &Resolved) {
    assert!(!r.enabled(&TEST_DECLS[0]));
    RUNS.fetch_add(1, Ordering::SeqCst);
}
fn fallback_is(expected: &'static str, r: &Resolved) {
    assert_eq!(r.choice(&TEST_DECLS[1]), Some(expected));
    RUNS.fetch_add(1, Ordering::SeqCst);
}
fn fb_new(r: &Resolved) {
    fallback_is("new", r);
}
fn fb_old(r: &Resolved) {
    fallback_is("old", r);
}
fn fb_older(r: &Resolved) {
    fallback_is("older", r);
}

const fn case(flag: usize, value: FlagValue, check: fn(&Resolved)) -> FlagCase {
    FlagCase {
        flag: TEST_DECLS[flag],
        value,
        check,
    }
}

const FULL: &[FlagCase] = &[
    case(0, FlagValue::Bool(true), kill_on),
    case(0, FlagValue::Bool(false), kill_off),
    case(1, FlagValue::Choice("new"), fb_new),
    case(1, FlagValue::Choice("old"), fb_old),
    case(1, FlagValue::Choice("older"), fb_older),
];

#[test]
fn a_full_set_has_no_gaps_and_every_case_runs_with_its_value_in_force() {
    assert_eq!(gaps(TEST_DECLS, FULL), Gaps::default());
    let before = RUNS.load(Ordering::SeqCst);
    run(TEST_DECLS, FULL);
    assert_eq!(RUNS.load(Ordering::SeqCst) - before, FULL.len());
}

#[test]
fn a_flag_tested_only_at_its_default_is_reported_at_each_retreat() {
    let only_defaults = [
        case(0, FlagValue::Bool(true), kill_on),
        case(1, FlagValue::Choice("new"), fb_new),
    ];
    assert_eq!(
        gaps(TEST_DECLS, &only_defaults).untested,
        vec![
            (KILL, FlagValue::Bool(false)),
            (FALLBACK, FlagValue::Choice("old")),
            (FALLBACK, FlagValue::Choice("older")),
        ]
    );
}

#[test]
fn a_flag_tested_only_at_its_retreat_is_reported_at_its_default() {
    let only_off = [case(0, FlagValue::Bool(false), kill_off)];
    let g = gaps(&TEST_DECLS[..1], &only_off);
    assert_eq!(g.untested, vec![(KILL, FlagValue::Bool(true))]);
}

#[test]
fn no_cases_at_all_report_every_value() {
    assert_eq!(gaps(TEST_DECLS, &[]).untested.len(), 5);
}

#[test]
fn a_case_for_an_undeclared_flag_or_value_is_stray() {
    let stale = [
        case(0, FlagValue::Bool(true), kill_on),
        case(1, FlagValue::Choice("gone"), fb_new),
    ];
    let g = gaps(&TEST_DECLS[..1], &stale);
    assert_eq!(g.stray, vec![(FALLBACK, FlagValue::Choice("gone"))]);
    let g = gaps(TEST_DECLS, &stale);
    assert_eq!(g.stray, vec![(FALLBACK, FlagValue::Choice("gone"))]);
}

#[test]
#[should_panic(expected = "assertion")]
fn a_case_whose_check_fails_fails_the_run() {
    // A check that asserts the wrong behaviour for its value.
    let wrong = [case(0, FlagValue::Bool(false), kill_on)];
    run(TEST_DECLS, &wrong);
}
