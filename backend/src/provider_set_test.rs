//! The provider set: order, one gate per known id, origin overrides.

use super::*;

#[test]
fn the_shipped_set_walks_hianime_alone() {
    // anidb.app redirects every page to an unrelated site; asked, it
    // can only fail over. The walk asks hianime and nothing else.
    let set = ProviderSet::shipped();
    assert_eq!(set.order(), &[ProviderId::Hianime]);
}

/// Whether `id` ships — written as a match with no wildcard, so a
/// variant added to [`ProviderId`] does not compile here until someone
/// says which list it belongs in. Without this, a variant left out of
/// [`ProviderId::ALL`] compiles and then has no gate.
fn ships(id: ProviderId) -> bool {
    match id {
        ProviderId::Hianime => true,
        ProviderId::TestA | ProviderId::TestB => false,
    }
}

#[test]
fn every_variant_is_listed_where_it_belongs() {
    let listed: Vec<ProviderId> = ProviderId::ALL
        .iter()
        .chain(ProviderId::TEST)
        .copied()
        .collect();
    for id in &listed {
        assert_eq!(ProviderId::ALL.contains(id), ships(*id), "{id:?}");
        assert_eq!(ProviderId::TEST.contains(id), !ships(*id), "{id:?}");
    }
    let set = ProviderSet::for_tests();
    for id in listed {
        let _ = set.gate(id);
    }
}

#[test]
fn every_shipped_providers_label_is_the_identifier_it_serializes_as() {
    // The renderer names a provider by its serialized identifier, and
    // the title-match routes read that value with `from_label`: a
    // provider whose two names differed would store its mappings
    // under the legacy segment.
    for id in ProviderId::ALL {
        let identifier = serde_json::to_value(id).expect("serializes");
        assert_eq!(identifier.as_str(), Some(id.label()), "{id:?}");
        assert_eq!(ProviderId::from_label(id.label()), Some(*id), "{id:?}");
    }
}

#[test]
fn every_known_provider_has_a_gate_whether_or_not_it_is_ordered() {
    // A row written by a provider the order no longer lists still
    // names it, and the cache's read rule asks that gate.
    let set = ProviderSet::listing(vec![ProviderId::Hianime]);
    for id in ProviderId::ALL {
        assert!(!set.gate(*id).is_open(), "{id:?} has a fresh gate");
    }
    assert!(
        !std::ptr::eq(set.gate(ProviderId::TestA), set.gate(ProviderId::Hianime)),
        "one gate per provider, never shared"
    );
}

#[test]
fn a_clone_shares_the_gates() {
    // AppState is cloned per request; a breaker that opened on one
    // clone has opened on every clone.
    let set = ProviderSet::for_tests();
    let other = set.clone();
    assert!(std::ptr::eq(
        set.gate(ProviderId::TestA),
        other.gate(ProviderId::TestA)
    ));
}

#[test]
fn origins_are_absent_until_a_test_points_a_provider_somewhere() {
    let mut set = ProviderSet::for_tests();
    assert_eq!(set.origin(ProviderId::TestA), None);
    assert_eq!(set.origin(ProviderId::Hianime), None);
    set.set_origin(ProviderId::Hianime, "http://127.0.0.1:1");
    assert_eq!(set.origin(ProviderId::Hianime), Some("http://127.0.0.1:1"));
    assert_eq!(
        set.origin(ProviderId::TestA),
        None,
        "an override is per provider"
    );
    let chained = set.with_origin(ProviderId::TestA, "http://127.0.0.1:2");
    assert_eq!(
        chained.origin(ProviderId::TestA),
        Some("http://127.0.0.1:2")
    );
}

#[test]
fn the_order_can_be_replaced_without_touching_gates_or_origins() {
    let mut set = ProviderSet::for_tests().with_origin(ProviderId::TestA, "http://127.0.0.1:1");
    let gate_before = set.gate(ProviderId::TestA) as *const ScraperGate;
    set.set_order(vec![ProviderId::Hianime, ProviderId::TestA]);
    assert_eq!(set.order(), &[ProviderId::Hianime, ProviderId::TestA]);
    assert!(std::ptr::eq(set.gate(ProviderId::TestA), gate_before));
    assert_eq!(set.origin(ProviderId::TestA), Some("http://127.0.0.1:1"));
}
