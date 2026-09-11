//! Property coverage for the provider seam's pure mapping: the label
//! each provider id carries into the renderer's progress copy.

use super::ProviderId;
use proptest::prelude::*;

/// Every provider the seam knows, and the label each one is pinned
/// to. A provider added to the enum without a row here is a compile
/// error in the match below — the point: the next provider cannot
/// drift the user-visible attribution silently.
fn provider() -> impl Strategy<Value = ProviderId> {
    prop_oneof![Just(ProviderId::Anidb), Just(ProviderId::Hianime)]
}

fn pinned_label(id: ProviderId) -> &'static str {
    match id {
        ProviderId::Anidb => "anidb.app",
        ProviderId::Hianime => "hianime",
    }
}

proptest! {
    /// The label is the provider's pinned one — a stable, non-empty
    /// string the renderer interpolates into "Searching {provider}…".
    #[test]
    fn each_provider_carries_its_pinned_label(id in provider()) {
        prop_assert_eq!(id.label(), pinned_label(id));
        prop_assert!(!id.label().is_empty());
        prop_assert!(!id.label().contains(char::is_whitespace));
    }
}
