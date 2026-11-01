//! Property coverage for the cache revisions, beside
//! `account_revisions.rs`: against a plain log of what happened, which
//! rows a refresh keeps and whether it is cached at all.

use std::collections::HashSet;

use proptest::prelude::*;

use super::CacheRevisions;
use crate::account::provider::ProviderKind;

#[derive(Debug, Clone)]
enum Step {
    /// A refresh of the provider starts.
    Start(bool),
    /// A row of the provider is written.
    Write(bool, u32),
    /// The started refresh at this index (modulo those not yet cached
    /// or dropped) arrives.
    Arrive(usize),
}

fn kind(mal: bool) -> ProviderKind {
    if mal {
        ProviderKind::MyAnimeList
    } else {
        ProviderKind::AniList
    }
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        any::<bool>().prop_map(Step::Start),
        (any::<bool>(), 0u32..4).prop_map(|(m, id)| Step::Write(m, id)),
        any::<usize>().prop_map(Step::Arrive),
    ]
}

proptest! {
    #[test]
    fn a_refresh_keeps_exactly_the_rows_written_since_it_started_and_never_lands_behind_a_later_one(
        steps in proptest::collection::vec(step(), 1..40)
    ) {
        let revisions = CacheRevisions::default();
        // The log: every event in order, by its position.
        let mut log: Vec<Step> = Vec::new();
        // Refreshes out: (provider, position started, revision).
        let mut out: Vec<(bool, usize, u64)> = Vec::new();
        // Per provider, the position of the latest-started refresh cached.
        let mut cached_from: [Option<usize>; 2] = [None, None];
        for s in steps {
            match s {
                Step::Start(m) => {
                    let since = revisions.start_refresh(kind(m));
                    out.push((m, log.len(), since));
                }
                Step::Write(m, id) => {
                    revisions.write_row(kind(m), id, || ());
                }
                Step::Arrive(i) => {
                    if out.is_empty() {
                        continue;
                    }
                    let (m, at, since) = out.remove(i % out.len());
                    let later_cached = cached_from[usize::from(m)].is_some_and(|c| c > at);
                    let expected: HashSet<u32> = log[at..]
                        .iter()
                        .filter_map(|e| match e {
                            Step::Write(wm, id) if *wm == m => Some(*id),
                            _ => None,
                        })
                        .collect();
                    let mut kept = None;
                    let cached = revisions.cache_refresh(kind(m), since, |keep| {
                        kept = Some(keep.clone());
                        Ok::<(), ()>(())
                    });
                    prop_assert_eq!(cached.is_none(), later_cached);
                    if !later_cached {
                        prop_assert_eq!(kept, Some(expected));
                        cached_from[usize::from(m)] = Some(at);
                    }
                }
            }
            log.push(s);
        }
    }
}

/// A removal's evidence of absence weighed against cached answers by
/// when each was asked, not when it arrived.
mod absent_evidence {
    use super::CacheRevisions;
    use crate::account::provider::ProviderKind;

    const K: ProviderKind = ProviderKind::AniList;

    #[test]
    fn an_absent_read_with_nothing_newer_clears_the_row_once() {
        let r = CacheRevisions::default();
        let read = r.start_read(K);
        r.note_read(K, 21, read, true);
        assert_eq!(r.clear_if_still_absent(K, 21, || ()), Some(()));
        assert_eq!(r.clear_if_still_absent(K, 21, || ()), None);
    }

    #[test]
    fn an_absent_read_sent_before_a_cached_refresh_cannot_clear_its_row() {
        let r = CacheRevisions::default();
        let read = r.start_read(K);
        let since = r.start_refresh(K);
        r.cache_refresh(K, since, |_| Ok::<(), ()>(()))
            .expect("the latest refresh")
            .expect("cached");
        // The read's answer arrives after the refresh was cached.
        r.note_read(K, 21, read, true);
        assert_eq!(r.clear_if_still_absent(K, 21, || ()), None);
    }

    #[test]
    fn an_absent_read_sent_before_a_write_cannot_clear_its_row() {
        let r = CacheRevisions::default();
        let read = r.start_read(K);
        r.write_row(K, 21, || ());
        r.note_read(K, 21, read, true);
        assert_eq!(r.clear_if_still_absent(K, 21, || ()), None);
    }

    #[test]
    fn an_older_absent_read_arriving_after_a_newer_listed_one_cannot_clear() {
        let r = CacheRevisions::default();
        let older = r.start_read(K);
        let newer = r.start_read(K);
        r.note_read(K, 21, newer, false);
        r.note_read(K, 21, older, true);
        assert_eq!(r.clear_if_still_absent(K, 21, || ()), None);
    }

    #[test]
    fn a_newer_absent_read_still_clears_after_an_older_listed_one() {
        let r = CacheRevisions::default();
        let older = r.start_read(K);
        let newer = r.start_read(K);
        r.note_read(K, 21, older, false);
        r.note_read(K, 21, newer, true);
        assert_eq!(r.clear_if_still_absent(K, 21, || ()), Some(()));
    }
}
