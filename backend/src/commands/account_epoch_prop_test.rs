//! Property coverage for the connection epochs, beside
//! `account_epoch.rs`.

use super::AccountEpochs;
use crate::account::provider::ProviderKind;

/// One step a run of requests and disconnects can take.
#[derive(Debug, Clone)]
enum Step {
    /// A request for the provider arrives: it notes the epoch.
    Arrive(bool),
    /// The request at this index (modulo those arrived) writes through.
    Write(usize),
    /// The provider's cache is purged.
    Purge(bool),
}

fn kind(mal: bool) -> ProviderKind {
    if mal {
        ProviderKind::MyAnimeList
    } else {
        ProviderKind::AniList
    }
}

fn step() -> impl proptest::strategy::Strategy<Value = Step> {
    use proptest::prelude::*;
    prop_oneof![
        any::<bool>().prop_map(Step::Arrive),
        any::<usize>().prop_map(Step::Write),
        any::<bool>().prop_map(Step::Purge),
    ]
}

proptest::proptest! {
    /// However requests, write-throughs and purges interleave, a write
    /// lands exactly when no purge of its own provider came since its
    /// request arrived; a purge of the other provider never stops it.
    #[test]
    fn a_write_lands_only_in_the_connection_it_arrived_in(
        steps in proptest::collection::vec(step(), 0..60),
    ) {
        let epochs = AccountEpochs::default();
        // Each arrived request: its provider, its epoch, and how many
        // purges of that provider had happened when it arrived.
        let mut arrived: Vec<(bool, u64, usize)> = Vec::new();
        let mut purges = [0usize; 2];
        for s in steps {
            match s {
                Step::Arrive(mal) => {
                    arrived.push((mal, epochs.current(kind(mal)), purges[usize::from(mal)]));
                }
                Step::Purge(mal) => {
                    epochs.purge(kind(mal), || ());
                    purges[usize::from(mal)] += 1;
                }
                Step::Write(i) if !arrived.is_empty() => {
                    let (mal, began, purges_then) = arrived[i % arrived.len()];
                    let landed = epochs.write_if_current(kind(mal), began, || ()).is_some();
                    proptest::prop_assert_eq!(landed, purges[usize::from(mal)] == purges_then);
                }
                Step::Write(_) => {}
            }
        }
    }
}
