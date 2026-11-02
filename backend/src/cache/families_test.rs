//! The registry's decision over a row, and what the registry may hold.

use super::*;
use proptest::prelude::*;

fn family(prefix: &str, read_after_expiry: bool) -> Family {
    let f = Family::prefix(prefix);
    if read_after_expiry {
        f.read_after_expiry()
    } else {
        f
    }
}

#[test]
fn a_row_no_family_declares_is_undeclared_however_fresh() {
    let families = [family("availability:v25:", true)];
    assert_eq!(
        fate("availability:v24:1:sub", false, &families),
        Fate::Undeclared
    );
}

#[test]
fn an_expired_row_goes_unless_its_family_is_read_past_expiry() {
    let families = [
        family("aniskip:v2:", false),
        family("title-match:v3:", true),
    ];
    assert_eq!(fate("aniskip:v2:1:2:3", true, &families), Fate::Expired);
    assert_eq!(fate("aniskip:v2:1:2:3", false, &families), Fate::Keep);
    assert_eq!(
        fate("title-match:v3:legacy:x:c1", true, &families),
        Fate::Keep
    );
}

#[test]
fn a_whole_key_family_holds_its_key_alone() {
    let families = [Family::key("kitsu:v2:trending")];
    assert_eq!(fate("kitsu:v2:trending", false, &families), Fate::Keep);
    assert_eq!(
        fate("kitsu:v2:trending_x", false, &families),
        Fate::Undeclared
    );
}

#[test]
fn every_key_this_build_builds_is_declared() {
    // One key from each builder reachable here: the registry is where
    // they take their prefixes from, so each must land in a family.
    for key in [
        crate::commands::kitsu::allmanga_kitsu_key("hianime:x-1"),
        crate::commands::kitsu::allmanga_kitsu_played_key("hianime:x-1"),
        crate::commands::kitsu::watched_at_key("hianime:x-1"),
        crate::commands::kitsu::anime_detail_key("12"),
        crate::commands::kitsu::title_match_prefix(None, "x") + "1",
        crate::commands::availability::cache_key("12", "sub"),
        crate::commands::listing_tags::key("hianime:x-1"),
        crate::commands::play_resolution_cache::cache_key(
            "T", "sub", "best", "1", None, None, None,
        ),
    ] {
        assert!(declared(&key), "{key}");
    }
}

/// A family's name with its version taken out: `availability:v25:`
/// reads `availability:v:`. Two families with one name are one family
/// declared at two versions.
fn unversioned(prefix: &str) -> String {
    prefix
        .split(':')
        .map(|part| match part.strip_prefix('v') {
            Some(digits) if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => "v",
            _ => part,
        })
        .collect::<Vec<_>>()
        .join(":")
}

#[test]
fn no_family_is_declared_at_two_versions() {
    // A bump that left the version before it declared would keep that
    // version's rows for good.
    let mut seen = std::collections::HashMap::new();
    for f in live() {
        if let Some(other) = seen.insert(unversioned(&f.prefix), f.prefix.clone()) {
            panic!("{other} and {} are one family at two versions", f.prefix);
        }
    }
}

#[test]
fn no_family_holds_another_familys_keys() {
    // A prefix inside another would let one family's rows stand for
    // the other's.
    let families = live();
    for a in &families {
        for b in &families {
            if a != b && !a.whole_key {
                assert!(
                    !b.prefix.starts_with(&a.prefix),
                    "{} holds {}",
                    a.prefix,
                    b.prefix
                );
            }
        }
    }
}

fn arb_family() -> impl Strategy<Value = Family> {
    ("[a-c]{1,3}:v[1-3]:", any::<bool>(), any::<bool>()).prop_map(|(p, whole, rae)| {
        let f = if whole {
            Family::key(p)
        } else {
            Family::prefix(p)
        };
        if rae {
            f.read_after_expiry()
        } else {
            f
        }
    })
}

proptest! {
    #[test]
    fn a_rows_fate_follows_its_families(
        families in prop::collection::vec(arb_family(), 0..5),
        key in "[a-c]{1,3}:v[1-3]:[a-c]{0,3}",
        expired in any::<bool>(),
    ) {
        let holding: Vec<&Family> = families.iter().filter(|f| f.holds(&key)).collect();
        let expected = if holding.is_empty() {
            Fate::Undeclared
        } else if expired && !holding.iter().any(|f| f.read_after_expiry) {
            Fate::Expired
        } else {
            Fate::Keep
        };
        prop_assert_eq!(fate(&key, expired, &families), expected);
    }

    #[test]
    fn a_fresh_declared_row_is_always_kept(
        families in prop::collection::vec(arb_family(), 1..5),
        rest in "[a-c]{0,3}",
    ) {
        let f = &families[0];
        let key = if f.whole_key { f.prefix.clone() } else { format!("{}{rest}", f.prefix) };
        prop_assert_eq!(fate(&key, false, &families), Fate::Keep);
    }

    #[test]
    fn unversioning_keeps_every_other_part(name in "[a-z-]{1,8}", n in 1u32..500, rest in "[a-z-]{0,6}") {
        prop_assert_eq!(unversioned(&format!("{name}:v{n}:{rest}")), format!("{name}:v:{rest}"));
    }
}
