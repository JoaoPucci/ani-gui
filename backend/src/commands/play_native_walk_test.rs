use crate::commands::play_native_resolve::NativeError;
use crate::commands::play_native_walk::pick_native_walk;
use crate::error::AniError;
use crate::scraper::table_provider::{Answer, TableProvider};

use super::super::play_native_test_provider::{scripted, Pool, THE_SHOW};
use crate::commands::play_native::PickedShow;

// ---- pick_native_walk: the availability probes' pick half, driven
// directly so each verdict arm is pinned where it lives rather than
// only through the probes that consume it. ----

async fn walk(
    provider: &TableProvider,
    title: &str,
    alts: &[String],
    expected: Option<u32>,
) -> std::result::Result<PickedShow, NativeError> {
    pick_native_walk(provider, title, alts, expected, None, None).await
}

#[tokio::test]
async fn the_pick_walk_reports_the_all_clean_miss() {
    // Every alias answered a genuine no-results page: the one verdict
    // that proves absence, and the only one availability may persist.
    let provider = scripted(&[]);
    let ne = walk(
        &provider,
        "unknown show",
        &["other name".to_string()],
        Some(3),
    )
    .await
    .expect_err("nothing matches");
    assert!(ne.clean_miss, "all-clean absence is the persistable miss");
    assert!(matches!(ne.error, AniError::NoResults));
    assert!(
        ne.failed_at.is_none(),
        "no attempt failed, nothing to stamp"
    );
    assert_eq!(provider.asked().len(), 2, "both aliases were asked");
}

#[tokio::test]
async fn the_pick_walk_recovers_through_a_later_alias() {
    // The canonical title lands a pool the pick rejects on episode
    // distance (seven listed against three expected). That rejection
    // is a verdict about THAT pool, so the walk moves to the fallback
    // alias and picks the real show there.
    let provider = scripted(&[
        ("wrong cour", Pool::Hits(&[("wrong-cour-99", "Wrong Cour")])),
        ("the show", THE_SHOW),
    ]);
    let picked = walk(&provider, "wrong cour", &["the show".to_string()], Some(3))
        .await
        .expect("the alias carries the real show");
    assert_eq!(picked.hit.slug, "the-show-77");
}

#[tokio::test]
async fn the_pick_walk_stops_typed_on_an_interstitial() {
    // A challenge page is the provider refusing the CLIENT, not
    // answering about the show — pressing later aliases through it
    // would hammer a blocking provider. The walk stops typed.
    let provider = scripted(&[("blocked", Pool::Blocked)]);
    let ne = walk(
        &provider,
        "blocked show",
        &["never asked".to_string()],
        Some(3),
    )
    .await
    .expect_err("the block stops the walk");
    assert!(matches!(ne.error, AniError::Upstream { .. }));
    assert!(!ne.clean_miss);
    assert_eq!(
        provider.asked().len(),
        1,
        "the walk must not press later aliases through a block"
    );
}

#[tokio::test]
async fn the_pick_walk_marks_answered_dead_ends_non_persistable() {
    // A pool whose every candidate answers not-found (stale slugs) is
    // dead candidates on a healthy provider: NoResults, but NOT the
    // persistable clean miss — the next probe should look again.
    // "Ghost", the title searched: a listing that could have won.
    let provider = scripted(&[("ghost", Pool::Hits(&[("ghost-1", "Ghost")]))]);
    let ne = walk(&provider, "ghost", &[], Some(3))
        .await
        .expect_err("dead candidates resolve nothing");
    assert!(matches!(ne.error, AniError::NoResults));
    assert!(
        !ne.clean_miss,
        "an answered dead end proves nothing about the show's absence"
    );
    assert!(
        !ne.refused_by_name,
        "a dead listing may have been the show's: no refusal by name"
    );
}

#[tokio::test]
async fn the_pick_walk_says_when_every_dead_end_was_a_refusal_by_name() {
    // An entry with a count and no year takes only a hit named for it:
    // "Some Other" fits the count and is named by nothing, so the pool
    // is refused on how its hit is named — the answered dead end, and a
    // refusal by name, which the verdict says.
    let provider = scripted(&[("the show", Pool::Hits(&[("the-show-77", "Some Other")]))]);
    let ne = walk(&provider, "the show", &[], Some(3))
        .await
        .expect_err("refused by name");
    assert!(matches!(ne.error, AniError::NoResults));
    assert!(!ne.clean_miss);
    assert!(ne.refused_by_name, "every dead end was a refusal by name");
}

#[tokio::test]
async fn a_dead_listing_beside_a_refusal_is_no_refusal_by_name() {
    // One alias's pool is refused by name, the other's candidate
    // answers its listing not-found — "Ghost", the alias itself, which
    // could have won: the dead one may have been the show, so the walk
    // does not say the catalogue lists nothing named for it.
    let provider = scripted(&[
        ("the show", Pool::Hits(&[("the-show-77", "Some Other")])),
        ("ghost", Pool::Hits(&[("ghost-1", "Ghost")])),
    ]);
    let ne = walk(&provider, "the show", &["ghost".to_string()], Some(3))
        .await
        .expect_err("refused and dead");
    assert!(matches!(ne.error, AniError::NoResults));
    assert!(!ne.clean_miss);
    assert!(!ne.refused_by_name);
}

/// A dead listing withdraws a refusal only when it could have won —
/// when the rule that reads how a hit is named would have let it stand
/// had it answered. Every pool here holds nothing but dead listings
/// that rule refuses: "Other Entry", which no title of the entry names
/// nor shares a name with,
/// for an entry with a count and no year (the count-alone rule), one
/// video long (the single-video rule), or dated with a count (the
/// dated-name rule). Beside another alias's refusal by name, the walk
/// is the refusal by name; alone, the pool is that refusal too, never
/// the clean miss.
#[tokio::test]
async fn a_dead_listing_no_name_vouches_for_withdraws_no_refusal() {
    for (rule, expected, year) in [
        ("count-alone", Some(3), None),
        ("single-video", Some(1), None),
        ("dated-name", Some(3), Some(2020)),
    ] {
        let alone = scripted(&[("ghost", Pool::Hits(&[("ghost-1", "Other Entry")]))]);
        let ne = pick_native_walk(&alone, "ghost", &[], expected, year, None)
            .await
            .expect_err("nothing picked");
        assert!(!ne.clean_miss, "{rule}: alone");
        assert!(
            ne.refused_by_name,
            "{rule}: alone, the dead hits are refused by name"
        );

        let beside = scripted(&[
            ("the show", Pool::Hits(&[("the-show-77", "Some Other")])),
            ("ghost", Pool::Hits(&[("ghost-1", "Other Entry")])),
        ]);
        let ne = pick_native_walk(
            &beside,
            "the show",
            &["ghost".to_string()],
            expected,
            year,
            None,
        )
        .await
        .expect_err("nothing picked");
        assert!(!ne.clean_miss, "{rule}");
        assert!(
            ne.refused_by_name,
            "{rule}: the dead listing could not have won"
        );
    }
}

#[tokio::test]
async fn the_pick_walk_aggregates_transport_deaths_with_a_stamp() {
    // One alias died on transport, another answered clean: the clean
    // page cannot prove absence past weather, so the aggregate is the
    // transient Network verdict — stamped with the provider's own
    // per-attempt start, exactly as the resolve walk stamps its.
    let stamp = tokio::time::Instant::now();
    let provider = TableProvider::new()
        .hits("dead alias", Answer::fails(|| AniError::Network))
        .stamped(stamp);
    let ne = pick_native_walk(
        &provider,
        "dead alias",
        &["clean alias".to_string()],
        None,
        None,
        None,
    )
    .await
    .expect_err("weather is not a verdict");
    assert!(matches!(ne.error, AniError::Network));
    assert!(!ne.clean_miss);
    assert_eq!(
        ne.failed_at,
        Some(stamp),
        "the stamp is the provider's per-attempt start"
    );
}

/// Every alias's search answered a page the client could not read:
/// the site answered, so the verdict is the parse failure under its
/// own name — stamped like any transient verdict — and never the
/// transport failure, whose copy tells the user to check a connection
/// the site answered on. The range download's start resolves through
/// this walk, so its dock row was that copy until this held here as
/// it does in the resolve walk.
#[tokio::test]
async fn the_pick_walk_reports_an_unreadable_search_as_itself_not_as_the_connection() {
    let stamp = tokio::time::Instant::now();
    let unreadable = || {
        Answer::fails(|| AniError::ParseFailed {
            detail: "the page is not the search shape".into(),
        })
    };
    let provider = TableProvider::new()
        .hits("title", unreadable())
        .hits("alias", unreadable())
        .stamped(stamp);
    let ne = walk(&provider, "title", &["alias".to_string()], None)
        .await
        .expect_err("nothing was read");
    assert!(
        matches!(ne.error, AniError::ParseFailed { .. }),
        "the failure met, under its own name: {:?}",
        ne.error
    );
    assert!(!ne.clean_miss);
    assert_eq!(ne.failed_at, Some(stamp));
}

/// Of a search that died and a search the client could not read, the
/// one that says the site answered is the verdict, whichever alias
/// came first — the same rule the resolve walk applies.
#[tokio::test]
async fn the_pick_walk_keeps_the_failure_that_says_most_across_aliases() {
    let unreadable = || {
        Answer::fails(|| AniError::ParseFailed {
            detail: "the page is not the search shape".into(),
        })
    };
    for (first, second) in [
        (Answer::fails(|| AniError::Network), unreadable()),
        (unreadable(), Answer::fails(|| AniError::Network)),
    ] {
        let provider = TableProvider::new()
            .hits("title", first)
            .hits("alias", second);
        let ne = walk(&provider, "title", &["alias".to_string()], None)
            .await
            .expect_err("nothing was read");
        assert!(
            matches!(ne.error, AniError::ParseFailed { .. }),
            "the unreadable answer outranks the dropped connection: {:?}",
            ne.error
        );
    }
}

#[tokio::test]
async fn the_pick_walk_stops_typed_on_a_gate_refusal() {
    // The gate's refusal is its OWN answer: the walk surfaces it
    // typed and does not spend the alias list against a closed gate.
    let provider = TableProvider::new().search_default(Answer::fails(|| AniError::GateRefused));
    let ne = pick_native_walk(
        &provider,
        "anything",
        &["something else".to_string()],
        None,
        None,
        None,
    )
    .await
    .expect_err("the refusal stops the walk");
    assert!(matches!(ne.error, AniError::GateRefused));
    assert!(!ne.clean_miss);
    assert_eq!(
        provider.asked().len(),
        1,
        "one refused admit; later aliases never asked"
    );
}

/// A rule refusing a pool by name: what it is, the pool, the title
/// searched, the entry's count and year.
type RefusalCase = (&'static str, Pool, &'static str, Option<u32>, Option<u32>);

/// The walk's verdict for `title` with `expected` and `year`, the
/// search answering `pool`.
async fn refused_walk(
    pool: Pool,
    title: &str,
    expected: Option<u32>,
    year: Option<u32>,
) -> NativeError {
    let provider = scripted(&[(title, pool)]);
    pick_native_walk(&provider, title, &[], expected, year, None)
        .await
        .expect_err("refused")
}

#[tokio::test]
async fn every_rule_that_refuses_a_pool_by_name_is_a_refusal_by_name() {
    // `the-show-77` lists two regular episodes and a recap; each pool
    // below is refused on how its one hit is named, by a different
    // rule, and every verdict says so.
    let cases: [RefusalCase; 6] = [
        (
            "the title rule, with a count: a season the entry never names",
            Pool::Hits(&[("the-show-77", "The Show Season 2")]),
            "the show",
            Some(3),
            Some(2020),
        ),
        (
            "the title rule, without a count",
            Pool::Hits(&[("the-show-77", "The Show Season 2")]),
            "the show",
            None,
            None,
        ),
        (
            "the countless pick: a refused hit beside one nothing vouches for",
            Pool::Hits(&[
                ("the-show-77", "The Show Season 2"),
                ("other-99", "Some Other"),
            ]),
            "the show",
            None,
            None,
        ),
        (
            "the single-video rule",
            Pool::Hits(&[("the-show-77", "Some Other")]),
            "the show",
            Some(1),
            None,
        ),
        (
            "the count-alone rule",
            Pool::Hits(&[("the-show-77", "Some Other")]),
            "the show",
            Some(3),
            None,
        ),
        (
            "the dated-name rule",
            Pool::Hits(&[("the-show-77", "Some Other")]),
            "the show",
            Some(3),
            Some(2020),
        ),
    ];
    for (rule, pool, title, expected, year) in cases {
        let ne = refused_walk(pool, title, expected, year).await;
        assert!(
            matches!(ne.error, AniError::NoResults),
            "{rule}: {:?}",
            ne.error
        );
        assert!(!ne.clean_miss, "{rule}: persisted");
        assert!(ne.refused_by_name, "{rule}: not a refusal by name");
    }
}

/// A dead listing inside the refused pool is no refusal by name either:
/// "The Show"'s listing answers not-found — stale, perhaps the show's —
/// beside a hit refused on how it is named, with a count and without.
#[tokio::test]
async fn a_dead_listing_inside_a_refused_pool_is_no_refusal_by_name() {
    let cases: [RefusalCase; 2] = [
        (
            "with a count",
            Pool::Hits(&[("ghost-1", "The Show"), ("the-show-77", "Some Other")]),
            "the show",
            Some(3),
            None,
        ),
        (
            "without a count",
            Pool::Hits(&[
                ("ghost-1", "The Show"),
                ("the-show-77", "The Show Season 2"),
            ]),
            "the show",
            None,
            None,
        ),
    ];
    for (label, pool, title, expected, year) in cases {
        let ne = refused_walk(pool, title, expected, year).await;
        assert!(
            matches!(ne.error, AniError::NoResults),
            "{label}: {:?}",
            ne.error
        );
        assert!(!ne.clean_miss, "{label}: persisted");
        assert!(
            !ne.refused_by_name,
            "{label}: a dead listing may have been the show's"
        );
    }
}

/// A pool whose verdict is no refusal can still hold a listing that
/// could have won and answered not-found: "The Show" dead beside a live
/// "The Show" seven episodes long against three. That listing may be
/// the show's, so the pool is never the clean miss, and a refusal by
/// name in another alias's pool is no refusal by name either — with a
/// count, and without one, where the dead listing is the exact title.
#[tokio::test]
async fn a_dead_listing_in_any_pool_is_never_a_clean_miss_nor_a_refusal_by_name() {
    // Refused by name with a count (nothing names "Some Other") and
    // without one (the title rule refuses a season the entry never
    // names, leaving the countless pick nothing).
    let refused_counted: (&str, Pool) = ("refused", Pool::Hits(&[("the-show-77", "Some Other")]));
    let refused_countless: (&str, Pool) = (
        "refused",
        Pool::Hits(&[("the-show-77", "The Show Season 2")]),
    );
    for (label, pool, expected, refused) in [
        (
            "with a count",
            Pool::Hits(&[("ghost-1", "The Show"), ("the-show-99", "The Show")]),
            Some(3),
            refused_counted,
        ),
        (
            "without a count",
            Pool::Hits(&[("ghost-1", "The Show")]),
            None,
            refused_countless,
        ),
    ] {
        // The refused pool alone is a refusal by name: what the dead
        // listing beside it has to withdraw.
        let only = scripted(&[refused]);
        let ne = pick_native_walk(&only, "refused", &[], expected, None, None)
            .await
            .expect_err("refused");
        assert!(ne.refused_by_name, "{label}: the refused pool alone");
        let alone = scripted(&[("the show", pool)]);
        let ne = pick_native_walk(&alone, "the show", &[], expected, None, None)
            .await
            .expect_err("nothing picked");
        assert!(
            !ne.clean_miss,
            "{label}: a dead listing that could have won is no clean miss"
        );

        let beside = scripted(&[("the show", pool), refused]);
        let ne = pick_native_walk(
            &beside,
            "the show",
            &["refused".to_string()],
            expected,
            None,
            None,
        )
        .await
        .expect_err("nothing picked");
        assert!(!ne.clean_miss, "{label}");
        assert!(
            !ne.refused_by_name,
            "{label}: the dead listing may have been the show's"
        );
    }
}

/// A dead listing that is another entry of the same show — "Show",
/// the entry before "Show 2" — withdraws nothing: answered, it would be
/// scored out and leave the clean miss, so dead it does the same. Alone,
/// the pool is the clean miss; beside another alias's refusal by name,
/// the refusal stands. (The alias, "Other Name 2", names no part "Show"
/// could be the part before, so "Show" is probed rather than dropped.)
#[tokio::test]
async fn a_dead_listing_of_another_entry_withdraws_no_refusal() {
    let alone = scripted(&[("show 2", Pool::Hits(&[("x-1", "Show")]))]);
    let ne = pick_native_walk(&alone, "show 2", &[], Some(3), None, None)
        .await
        .expect_err("nothing picked");
    assert!(
        ne.clean_miss,
        "the entry before, dead, is the clean miss it is alive"
    );

    let beside = scripted(&[
        ("other name 2", Pool::Hits(&[("the-show-77", "Other")])),
        ("show 2", Pool::Hits(&[("x-1", "Show")])),
    ]);
    let ne = pick_native_walk(
        &beside,
        "show 2",
        &["other name 2".to_string()],
        Some(3),
        None,
        None,
    )
    .await
    .expect_err("nothing picked");
    assert!(
        beside
            .asked()
            .iter()
            .any(|a| matches!(a, crate::scraper::table_provider::Asked::Episodes(s) if s == "x-1")),
        "the entry before was probed"
    );
    assert!(!ne.clean_miss);
    assert!(
        ne.refused_by_name,
        "the entry before could never be the show"
    );
}

/// The hits of `(slug, title)` pairs, for a table of the test's own.
fn named_hits(entries: &[(&str, &str)]) -> Vec<crate::scraper::provider::BrowseHit> {
    super::super::play_native_test_provider::hits(entries)
}

/// A dead listing the titles refuse decides the pick exactly when, as
/// a broad listing's completing sibling, it would have had the broad
/// listing cut: "The Show Season 3 Part 2" dead beside "The Show Season
/// 3", seven episodes against three, from the entry's year — the span
/// table's W, which the entry heads and the sibling names a later part
/// of. Then the pool is the dead end, alone and beside a hit refused by
/// name in the same pool. For an undated entry no W exists, the year
/// confirming none, so the dead sibling decides nothing: beside the
/// refused hit the pool is the refusal by name it would have been.
#[tokio::test]
async fn a_dead_sibling_decides_the_pick_only_as_a_broad_listings_completion() {
    let wide = [
        ("the-show-99", "The Show Season 3"),
        ("ghost-1", "The Show Season 3 Part 2"),
    ];
    let with_refused = [
        ("the-show-99", "The Show Season 3"),
        ("ghost-1", "The Show Season 3 Part 2"),
        ("the-show-77", "Some Other"),
    ];
    for (label, entries, year, refused) in [
        ("dated, alone", &wide[..], Some(2020), false),
        (
            "dated, beside a refusal",
            &with_refused[..],
            Some(2020),
            false,
        ),
        ("undated, beside a refusal", &with_refused[..], None, true),
    ] {
        let site = TableProvider::counted_with_years(&[(99, 7, Some(2020)), (77, 2, Some(2020))])
            .hits("the show season 3", named_hits(entries));
        let ne = pick_native_walk(&site, "the show season 3", &[], Some(3), year, None)
            .await
            .expect_err("nothing picked");
        assert!(!ne.clean_miss, "{label}");
        assert_eq!(ne.refused_by_name, refused, "{label}");
    }
}

/// A dead listing the titles or the name rule refuse is a refused hit
/// the pool holds, as it would be had it answered within the count: a
/// pool of a live "Show" three episodes long against twelve and a dead
/// "Show Season 2" is the refusal by name, not the clean miss.
#[tokio::test]
async fn a_dead_refused_listing_beside_a_live_misfit_is_a_refusal_by_name() {
    let site = TableProvider::counted_with_years(&[(5, 3, None)]).hits(
        "show",
        named_hits(&[("show-5", "Show"), ("ghost-1", "Show Season 2")]),
    );
    let ne = pick_native_walk(&site, "show", &[], Some(12), None, None)
        .await
        .expect_err("nothing picked");
    assert!(!ne.clean_miss);
    assert!(ne.refused_by_name);
}
