use super::*;

fn hits_from(kinds: &[Option<&'static str>]) -> Vec<BrowseHit> {
    kinds
        .iter()
        .enumerate()
        .map(|(i, k)| BrowseHit {
            slug: format!("show-{i}"),
            title: format!("Show {i}"),
            kind: k.map(str::to_string),
        })
        .collect()
}

const KINDS: &[Option<&str>] = &[
    None,
    Some("Movie"),
    Some("movie"),
    Some("MOVIE"),
    Some("TV"),
    Some("OVA"),
    Some("Special"),
    Some("Web"),
];

fn kind_strategy() -> impl proptest::strategy::Strategy<Value = Option<&'static str>> {
    use proptest::strategy::Strategy as _;
    (0..KINDS.len()).prop_map(|i| KINDS[i])
}

proptest::proptest! {
    /// Over arbitrary pools, expectations and tag casing: survivors
    /// are always an order-preserving subsequence, unknown badges
    /// never exclude, a signal-less call is the identity, and when
    /// both the subtype and the badge name known categories the two
    /// must agree — with the count-derived movie exclusion applying
    /// only when the subtype gives no category.
    #[test]
    fn the_disproof_holds_over_arbitrary_pools(
        kinds in proptest::collection::vec(kind_strategy(), 0..10),
        expected in proptest::option::of(0u32..50),
        subtype in proptest::option::of(proptest::prop_oneof![
            proptest::strategy::Just("movie"),
            proptest::strategy::Just("Movie"),
            proptest::strategy::Just("MOVIE"),
            proptest::strategy::Just("TV"),
            proptest::strategy::Just("special"),
            proptest::strategy::Just("OVA"),
            proptest::strategy::Just("ONA"),
        ]),
    ) {
        fn cat(tag: &str) -> Option<&'static str> {
            match tag.to_ascii_lowercase().as_str() {
                "movie" => Some("movie"),
                "tv" => Some("tv"),
                "ova" => Some("ova"),
                "special" => Some("special"),
                "ona" | "web" => Some("ona"),
                _ => None,
            }
        }
        let hits = hits_from(&kinds);
        let out = format_survivors(&hits, expected, subtype);
        // Order-preserving subsequence, nothing fabricated.
        let mut cursor = 0;
        for s in &out {
            let found = hits[cursor..].iter().position(|h| h == s);
            proptest::prop_assert!(found.is_some(), "fabricated or reordered: {s:?}");
            cursor += found.unwrap() + 1;
        }
        let want = subtype.and_then(cat);
        for h in &hits {
            let have = h.kind.as_deref().and_then(cat);
            let survive = match (want, have) {
                (Some(w), Some(hc)) => w == hc,
                _ => {
                    let expects_non_movie =
                        want.is_none() && matches!(expected, Some(n) if n > 1);
                    !(expects_non_movie && have == Some("movie"))
                }
            };
            proptest::prop_assert_eq!(out.contains(h), survive, "hit {:?}", h);
        }
        if want.is_none() && !matches!(expected, Some(n) if n > 1) {
            proptest::prop_assert_eq!(&out, &hits);
        }
    }
}
