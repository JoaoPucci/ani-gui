//! Property coverage for the transport's pure helpers.
//!
//! Its own file rather than an append to `anidb_test.rs`, matching
//! the other property modules on this branch.

use super::{candidate_names, fetch_args, redacted_url};

proptest::proptest! {
    /// Expansion is exactly the suffix table applied in order: one
    /// candidate per suffix, each the name with that suffix appended,
    /// none invented and none dropped — an empty table yields no
    /// candidates at all.
    #[test]
    fn expansion_is_the_suffix_table_applied_in_order(
        name in ".*",
        suffixes in proptest::collection::vec(".{0,6}", 0..6),
    ) {
        let refs: Vec<&str> = suffixes.iter().map(String::as_str).collect();
        let expanded = candidate_names(&name, &refs);
        proptest::prop_assert_eq!(expanded.len(), suffixes.len());
        for (got, suffix) in expanded.iter().zip(&suffixes) {
            proptest::prop_assert_eq!(got, &format!("{name}{suffix}"));
        }
    }
}

proptest::proptest! {
    /// The URL is the operand and stays last, for every URL and either
    /// impersonation state. curl reads a bare argument as the transfer
    /// target, so a flag appended after it would be parsed for a
    /// *second* transfer rather than modifying this one.
    #[test]
    fn the_url_is_always_the_final_argument(
        url in ".*",
        target in proptest::option::of("[a-z]{1,12}[0-9]{0,4}"),
    ) {
        let args = fetch_args(&url, target.as_deref());
        proptest::prop_assert_eq!(args.last().map(String::as_str), Some(url.as_str()));
    }

    /// The URL appears exactly once. Neither the impersonation arm nor
    /// the cipher table may duplicate the operand or drop it — a second
    /// occurrence is a second transfer, and the body parse would then
    /// read two concatenated responses.
    #[test]
    fn the_url_is_carried_exactly_once(
        url in "https://[a-z]{1,10}\\.[a-z]{2,4}/[a-z0-9/-]{0,20}",
        target in proptest::option::of("[a-z]{1,12}[0-9]{0,4}"),
    ) {
        let args = fetch_args(&url, target.as_deref());
        let hits = args.iter().filter(|a| *a == &url).count();
        proptest::prop_assert_eq!(hits, 1);
    }

    /// `--impersonate` appears if and only if a target was given, and
    /// when it appears the very next argument is that target verbatim.
    /// A flag separated from its value would consume whatever followed.
    #[test]
    fn the_impersonate_flag_tracks_its_target(
        url in ".*",
        target in proptest::option::of("[a-z]{1,12}[0-9]{0,4}"),
    ) {
        let args = fetch_args(&url, target.as_deref());
        let at = args.iter().position(|a| a == "--impersonate");
        match &target {
            Some(t) => {
                let i = at.expect("a target must reach the child as a flag");
                proptest::prop_assert_eq!(args.get(i + 1).map(String::as_str), Some(t.as_str()));
            }
            None => proptest::prop_assert!(at.is_none()),
        }
    }

    /// Passing a target only ever adds the flag and its value — every
    /// other argument, and their order, is what the no-target call
    /// produces. This is what keeps the platform that already worked
    /// from drifting when the impersonating arm changes.
    #[test]
    fn a_target_adds_two_arguments_and_disturbs_nothing_else(
        url in ".*",
        target in "[a-z]{1,12}[0-9]{0,4}",
    ) {
        let plain = fetch_args(&url, None);
        let with = fetch_args(&url, Some(&target));
        proptest::prop_assert_eq!(with.len(), plain.len() + 2);
        let stripped: Vec<String> = with
            .iter()
            .enumerate()
            .filter(|(i, a)| {
                let flag = *a == "--impersonate";
                let value = *i > 0 && with[i - 1] == "--impersonate";
                !flag && !value
            })
            .map(|(_, a)| a.clone())
            .collect();
        proptest::prop_assert_eq!(stripped, plain);
    }
}

proptest::proptest! {
    /// Redaction never emits a query, a fragment, or any path
    /// segment past the keep threshold — whatever shape the URL
    /// takes. The signed stream tokens are the long segments; the
    /// query is where providers put signatures. Host and scheme
    /// survive so the log still names the failing endpoint.
    #[test]
    fn redaction_never_leaks_long_segments_or_queries(
        host in r"[a-z]{1,10}\.[a-z]{2,3}",
        segs in proptest::collection::vec("[a-zA-Z0-9_-]{1,80}", 0..6),
        query in proptest::option::of("[a-zA-Z0-9=&_-]{1,40}"),
        fragment in proptest::option::of("[a-zA-Z0-9_-]{1,20}"),
    ) {
        let mut url = format!("https://{host}/{}", segs.join("/"));
        if let Some(q) = &query {
            url.push('?');
            url.push_str(q);
        }
        if let Some(f) = &fragment {
            url.push('#');
            url.push_str(f);
        }
        let red = redacted_url(&url);
        proptest::prop_assert!(!red.contains('?') && !red.contains('#'));
        proptest::prop_assert!(red.starts_with("https://"));
        proptest::prop_assert!(red.contains(&host));
        for seg in &segs {
            if seg.len() > 32 {
                proptest::prop_assert!(
                    !red.contains(seg.as_str()),
                    "credential-length segment survived: {red}"
                );
            }
        }
    }

    /// Total on arbitrary input: anything unparseable comes back as
    /// the placeholder rather than passing through verbatim, and
    /// nothing panics.
    #[test]
    fn redaction_is_total_and_never_echoes_unparseable_input(any in "[^:]*") {
        // No scheme separator → not a parseable absolute URL.
        proptest::prop_assert_eq!(redacted_url(&any), "<unparseable url>");
    }
}
