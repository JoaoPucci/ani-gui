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

fn movie_shaped(kind: Option<&str>) -> bool {
    kind.is_some_and(|k| k.eq_ignore_ascii_case("movie"))
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
    /// Over arbitrary pools, expectations and subtype casing: the
    /// survivors are always an order-preserving subsequence, unknown
    /// badges never exclude, a signal-less call is the identity, and
    /// both directions read the badge case-insensitively — a movie
    /// expectation keeps exactly the unknown-or-movie cards, a
    /// series expectation drops exactly the movie-shaped ones.
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
        ]),
    ) {
        let hits = hits_from(&kinds);
        let out = format_survivors(&hits, expected, subtype);
        // Order-preserving subsequence, nothing fabricated.
        let mut cursor = 0;
        for s in &out {
            let found = hits[cursor..].iter().position(|h| h == s);
            proptest::prop_assert!(found.is_some(), "fabricated or reordered: {s:?}");
            cursor += found.unwrap() + 1;
        }
        let expects_movie = subtype.is_some_and(|s| s.eq_ignore_ascii_case("movie"));
        let expects_non_movie = matches!(expected, Some(n) if n > 1)
            || subtype.is_some_and(|s| !s.eq_ignore_ascii_case("movie"));
        if expects_movie {
            // Exactly the unknown-or-movie cards survive.
            for h in &hits {
                let should_survive = h.kind.is_none() || movie_shaped(h.kind.as_deref());
                proptest::prop_assert_eq!(out.contains(h), should_survive, "hit {:?}", h);
            }
        } else if expects_non_movie {
            // Exactly the movie-shaped cards drop, any casing.
            for h in &hits {
                proptest::prop_assert_eq!(
                    out.contains(h),
                    !movie_shaped(h.kind.as_deref()),
                    "hit {:?}",
                    h
                );
            }
        } else {
            // No signal: the identity, unknown badges and all.
            proptest::prop_assert_eq!(&out, &hits);
        }
    }
}
