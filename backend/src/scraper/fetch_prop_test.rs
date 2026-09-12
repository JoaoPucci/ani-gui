//! Property coverage for the transport's pure helpers.
//!
//! Its own file rather than an append to `anidb_test.rs`, matching
//! the other property modules on this branch.

use super::{candidate_names, fetch_args, redacted_url, scrub_stderr, FetchRequest};

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
        let args = fetch_args(&FetchRequest::get(url.as_str()), target.as_deref());
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
        let args = fetch_args(&FetchRequest::get(url.as_str()), target.as_deref());
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
        let args = fetch_args(&FetchRequest::get(url.as_str()), target.as_deref());
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
        let plain = fetch_args(&FetchRequest::get(url.as_str()), None);
        let with = fetch_args(&FetchRequest::get(url.as_str()), Some(&target));
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

proptest::proptest! {
    /// Scrubbing is exactly replace-every-echo: each occurrence of
    /// the operand becomes its redaction, and every byte around the
    /// echoes — curl's own diagnosis — survives verbatim. The token
    /// never survives, however many times curl printed it.
    #[test]
    fn scrubbing_replaces_every_echo_and_nothing_else(
        host in r"[a-z]{1,10}\.[a-z]{2,3}",
        token in "[a-zA-Z0-9]{33,80}",
        prefix in "[ -~]{0,40}",
        suffix in "[ -~]{0,40}",
        copies in 1usize..4,
    ) {
        let url = format!("https://{host}/stream/{token}/master.m3u8");
        let stderr = format!("{prefix}{}{suffix}", vec![url.clone(); copies].join(" "));
        let scrubbed = scrub_stderr(&stderr, &url);
        proptest::prop_assert!(!scrubbed.contains(&token));
        let expected = format!(
            "{prefix}{}{suffix}",
            vec![redacted_url(&url); copies].join(" ")
        );
        proptest::prop_assert_eq!(scrubbed, expected);
    }

    /// stderr that never echoed the operand passes through unchanged
    /// — the scrub must not corrupt unrelated diagnostics.
    #[test]
    fn stderr_without_the_operand_passes_through_untouched(
        stderr in "[ -~]{0,80}",
        host in r"[a-z]{1,10}\.[a-z]{2,3}",
        token in "[a-zA-Z0-9]{33,80}",
    ) {
        let url = format!("https://{host}/stream/{token}/master.m3u8");
        proptest::prop_assume!(!stderr.contains(&url));
        proptest::prop_assert_eq!(scrub_stderr(&stderr, &url), stderr);
    }
}

proptest::proptest! {
    /// Every header the request carries reaches curl as its own `-H`
    /// operand, `Name: value`, in the request's order; the URL still
    /// follows them all as the final argument; and taking the pairs
    /// back out leaves exactly the headerless argv.
    #[test]
    fn each_header_rides_its_own_flag_in_order_ahead_of_the_url(
        url in "https://[a-z]{1,10}\\.[a-z]{2,4}/[a-z0-9/-]{0,20}",
        headers in proptest::collection::vec(("[A-Za-z-]{1,20}", "[^\\r\\n]{0,40}"), 0..6),
        target in proptest::option::of("[a-z]{1,12}[0-9]{0,4}"),
    ) {
        let mut req = FetchRequest::get(url.as_str());
        for (name, value) in &headers {
            req = req.header(name.clone(), value.clone());
        }
        let args = fetch_args(&req, target.as_deref());
        let flags: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| *a == "-H")
            .map(|(i, _)| i)
            .collect();
        proptest::prop_assert_eq!(flags.len(), headers.len());
        for (i, (name, value)) in flags.iter().zip(&headers) {
            let want = format!("{name}: {value}");
            proptest::prop_assert_eq!(args.get(*i + 1).map(String::as_str), Some(want.as_str()));
            proptest::prop_assert!(*i + 1 < args.len() - 1, "a header operand must precede the URL");
        }
        proptest::prop_assert_eq!(args.last().map(String::as_str), Some(url.as_str()));
        let mut stripped = args.clone();
        for i in flags.iter().rev() {
            stripped.drain(*i..*i + 2);
        }
        proptest::prop_assert_eq!(stripped, fetch_args(&FetchRequest::get(url.as_str()), target.as_deref()));
    }
}
