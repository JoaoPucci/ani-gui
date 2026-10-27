//! Properties of what a replay names: the former number it hands the
//! session, the listing tags it checks that number against, and the
//! window title its handoff opens with.
//!
//! Strategies are boxed so the Windows test build's debug info stays
//! under the linker's PDB limit.

use super::{cached_launch_args, cached_watch};
use crate::commands::play_native_merge::{carries_tag, former_on_replay};
use proptest::prelude::*;

/// A hearing of this process's clock, fetched between `start` and `at`.
fn heard(start: u64, at: u64) -> crate::commands::listing_tags::Hearing {
    crate::commands::listing_tags::Hearing { run: 0, start, at }
}

/// A tag as a listing writes one: a whole number, or a fraction of one.
fn tag() -> BoxedStrategy<String> {
    prop_oneof![
        (0u32..200).prop_map(|n| n.to_string()),
        (0u32..200, prop_oneof![Just("5"), Just("25"), Just("75")])
            .prop_map(|(n, f)| format!("{n}.{f}")),
    ]
    .boxed()
}

fn tags() -> BoxedStrategy<Vec<String>> {
    proptest::collection::vec(tag(), 0..6).boxed()
}

/// A tag's value, for comparing two spellings of one number.
fn value(t: &str) -> f64 {
    t.parse().expect("a numeric tag")
}

proptest! {
    /// A number is carried exactly when some tag names the same value,
    /// however either is written: "12.5" and "12.50" are one number.
    #[test]
    fn a_number_is_carried_exactly_when_a_tag_has_its_value(
        others in tags(),
        number in tag(),
        listed in any::<bool>(),
        zeros in 0usize..3,
    ) {
        // Half the listings carry the number as the listing writes it.
        let mut listing = others;
        if listed {
            listing.push(number.clone());
        }
        let spelled = if number.contains('.') {
            format!("{number}{}", "0".repeat(zeros))
        } else {
            number.clone()
        };
        let expected = listing.iter().any(|t| value(t) == value(&number));
        prop_assert_eq!(carries_tag(&listing, &spelled), expected);
    }

    /// A tag that does not read as a number is carried only by the same
    /// text.
    #[test]
    fn a_text_tag_is_carried_only_by_the_same_text(
        listing in proptest::collection::vec("[a-z]{1,4}", 0..5),
        number in "[a-z]{1,4}",
    ) {
        prop_assert_eq!(carries_tag(&listing, &number), listing.contains(&number));
    }

    /// A replay names the kept number exactly when one was kept, the
    /// listing is known, and no tag of it has the number's value — and
    /// then names it as kept.
    #[test]
    fn a_kept_number_is_named_only_while_a_known_listing_leaves_it_free(
        kept in proptest::option::of(tag()),
        listing in proptest::option::of(tags()),
    ) {
        let named = former_on_replay(kept.as_deref(), listing.as_deref());
        let expected = match (&kept, &listing) {
            (Some(k), Some(l)) if !l.iter().any(|t| value(t) == value(k)) => Some(k.clone()),
            _ => None,
        };
        prop_assert_eq!(named, expected);
    }

    /// The tags a listing at hand recorded are the ones a replay reads
    /// back for that show key, and for no other.
    #[test]
    fn a_listings_tags_read_back_for_its_own_show_key(
        recorded in tags(),
        slug in "[a-z]{1,8}-[0-9]{1,4}",
    ) {
        let state = crate::commands::play::tests::state_with_proxy_origin();
        let show_key = format!("test-a:{slug}");
        crate::commands::listing_tags::put(&state, &show_key, &recorded, heard(1, 1));
        prop_assert_eq!(
            crate::commands::listing_tags::get(&state, &show_key),
            Some(recorded)
        );
        prop_assert_eq!(
            crate::commands::listing_tags::get(&state, &format!("test-b:{slug}")),
            None
        );
    }

    /// A cached handoff's window names the episode its resolve played,
    /// or the request on a row from before that was kept.
    #[test]
    fn a_cached_handoffs_window_names_the_episode_its_resolve_played(
        show in "[A-Za-z0-9 ]{1,20}",
        requested in tag(),
        played in proptest::option::of(tag()),
    ) {
        let cached = crate::commands::play_resolution_cache::CachedResolution {
            upstream_url: "https://cdn.example/x/master.m3u8".into(),
            referer: String::new(),
            media_kind: crate::proxy::MediaKind::Hls,
            show_id: "test-a:show-1".into(),
            show_title: "Show".into(),
            resolved_slot: Some(1),
            subtitles: Vec::new(),
            kitsu_id: None,
            versions: Vec::new(),
            version: None,
            played: played.clone(),
            former_episode: None,
            episode_unverified: false,
        };
        let args: crate::commands::play::PlayArgs = serde_json::from_value(serde_json::json!({
            "title": show,
            "episode": requested,
            "mode": "sub",
        }))
        .expect("args");
        let expected = played.unwrap_or_else(|| requested.clone());
        prop_assert_eq!(cached_watch(&cached, &args.episode).ep_no, expected.clone());
        let launch = cached_launch_args(cached, &args, &crate::config::Config::default());
        prop_assert_eq!(launch.title, Some(format!("{show} · ep {expected}")));
    }
}

/// Two walks of a show overlap: one picks the show, a later one picks it
/// again and records the listing it heard, and the first records its
/// older listing after that. The record keeps the later listing.
#[test]
fn an_older_listing_recorded_last_leaves_the_newer_one() {
    let state = crate::commands::play::tests::state_with_proxy_origin();
    let key = "test-a:show-1";
    crate::commands::listing_tags::put(&state, key, &["24.5".to_string()], heard(200, 200));
    crate::commands::listing_tags::put(&state, key, &[], heard(100, 100));
    assert_eq!(
        crate::commands::listing_tags::get(&state, key),
        Some(vec!["24.5".to_string()])
    );
}

/// A play's numbering repair runs once its resolve has finished, which
/// can be after another walk heard a newer listing of the same show. The
/// repair leaves that record — of the copy it played and of every other
/// version — as the newer walk wrote it.
#[test]
fn a_numbering_repair_finishing_late_leaves_a_newer_listing() {
    let state = crate::commands::play::tests::state_with_proxy_origin();
    let row = |id: u64, number: u32, tag: Option<&str>| crate::scraper::provider::EpisodeRef {
        id,
        number,
        number2: tag.map(str::to_string),
        former_tag: None,
        title: None,
    };
    // The newer walk heard its listing just now, before the repair runs.
    for key in ["test-a:show-uncut-1", "test-a:show-2"] {
        crate::commands::listing_tags::put(
            &state,
            key,
            &["2.5".to_string()],
            crate::commands::listing_tags::Begun::now().picked(),
        );
    }
    let native = crate::commands::play_native_resolve::NativeResolved {
        slug: "test-a:show-uncut-1".into(),
        title: "Show".into(),
        master_url: "https://cdn.example/x/master.m3u8".into(),
        episode_cap: Some(2),
        numbering_offset: 0,
        extra_tags: vec!["1.5".into()],
        resolved_slot: 1,
        resolved_tag: None,
        referer: None,
        subtitles: Vec::new(),
        provider: crate::scraper::provider::ProviderId::TestA,
        versions: Vec::new(),
        version: None,
        former_tags: Vec::new(),
        other_copies: vec![(
            "test-a:show-2".into(),
            vec![row(1, 1, None), row(2, 2, Some("1.25")), row(3, 3, None)],
        )],
        unread_past: None,
    };
    crate::commands::numbering_offset::retag_from_resolve(&state, &native);
    crate::commands::numbering_offset::retag_from_listing(
        &state,
        "test-a:show-2",
        &[row(1, 1, None), row(2, 2, Some("1.25"))],
    );
    for key in ["test-a:show-uncut-1", "test-a:show-2"] {
        assert_eq!(
            crate::commands::listing_tags::get(&state, key),
            Some(vec!["2.5".to_string()]),
            "{key}"
        );
    }
}

proptest! {
    /// Whatever order a show's listings are recorded in, the record holds
    /// the one heard last; among those heard at the same moment, the one
    /// written last.
    #[test]
    fn the_record_holds_the_listing_heard_last_whatever_the_write_order(
        hearings in proptest::collection::vec((0u64..20, tags()), 1..8),
    ) {
        let state = crate::commands::play::tests::state_with_proxy_origin();
        for (at, recorded) in &hearings {
            crate::commands::listing_tags::put(&state, "test-a:show-1", recorded, heard(*at, *at));
        }
        let latest = hearings.iter().map(|(at, _)| *at).max().expect("one");
        let expected = hearings
            .iter()
            .rev()
            .find(|(at, _)| *at == latest)
            .map(|(_, t)| t.clone());
        prop_assert_eq!(crate::commands::listing_tags::get(&state, "test-a:show-1"), expected);
    }
}

/// Records decided between: one of `run`, heard between `start` and
/// `at`, carrying `tags`.
fn record(run: u64, start: u64, at: u64, tags: &[&str]) -> crate::commands::listing_tags::Record {
    crate::commands::listing_tags::Record {
        heard: crate::commands::listing_tags::Hearing { run, start, at },
        tags: tags.iter().map(|t| (*t).to_string()).collect(),
    }
}

/// Two walks whose spans overlap cannot tell which fetched its listing
/// last. The record keeps the tags of both, over the span of both: a
/// former number either listing gives an episode is not named.
#[test]
fn overlapping_hearings_keep_both_listings_tags() {
    let stored = record(1, 10, 20, &["24.5"]);
    let new = record(1, 15, 25, &["12.5"]);
    let kept = crate::commands::listing_tags::next_record(Ok(Some(stored)), new).expect("written");
    assert_eq!(kept.heard.start, 10);
    assert_eq!(kept.heard.at, 25);
    let mut tags = kept.tags;
    tags.sort();
    assert_eq!(tags, vec!["12.5".to_string(), "24.5".to_string()]);
}

/// A listing a walk began fetching after the stored one was picked is
/// newer, and replaces it; one picked before the stored one began is
/// older, and is dropped.
#[test]
fn a_hearing_clearly_before_or_after_the_stored_one_is_ordered() {
    let stored = || record(1, 10, 20, &["24.5"]);
    let later =
        crate::commands::listing_tags::next_record(Ok(Some(stored())), record(1, 21, 30, &[]))
            .expect("written");
    assert!(later.tags.is_empty());
    assert_eq!(
        crate::commands::listing_tags::next_record(Ok(Some(stored())), record(1, 2, 9, &[])),
        None
    );
}

/// The stamps are this process's clock. One backend runs per cache
/// directory, so a record an earlier run wrote is older than any hearing
/// now, whatever its stamps read.
#[test]
fn a_record_an_earlier_run_wrote_is_replaced_by_any_hearing_now() {
    let stored = record(1, u64::MAX - 1, u64::MAX, &["24.5"]);
    let new = record(2, 5, 6, &[]);
    assert_eq!(
        crate::commands::listing_tags::next_record(Ok(Some(stored)), new.clone()),
        Some(new)
    );
}

/// A read of the store that fails says nothing of what is stored, which
/// may be a newer listing: nothing is written over it.
#[test]
fn a_failed_read_writes_nothing() {
    assert_eq!(
        crate::commands::listing_tags::next_record(Err(()), record(1, 1, 2, &[])),
        None
    );
}

/// A hearing of one run, for the properties below: its span, and its
/// tags.
fn hearing() -> BoxedStrategy<(u64, u64, Vec<String>)> {
    (0u64..50, 0u64..20, tags())
        .prop_map(|(start, len, tags)| (start, start + len, tags))
        .boxed()
}

/// Write `hearings` of one run, in order, through `next_record`.
fn written(hearings: &[(u64, u64, Vec<String>)]) -> Option<crate::commands::listing_tags::Record> {
    let mut stored = None;
    for (start, at, t) in hearings {
        let new = crate::commands::listing_tags::Record {
            heard: crate::commands::listing_tags::Hearing {
                run: 1,
                start: *start,
                at: *at,
            },
            tags: t.clone(),
        };
        if let Some(r) = crate::commands::listing_tags::next_record(Ok(stored.clone()), new) {
            stored = Some(r);
        }
    }
    stored
}

fn tag_set(
    r: &Option<crate::commands::listing_tags::Record>,
) -> std::collections::BTreeSet<String> {
    r.iter().flat_map(|r| r.tags.iter().cloned()).collect()
}

proptest! {
    /// Of two hearings, one heard wholly after the other ends as the
    /// record, whichever is written last. A run's stamps are all
    /// distinct, so "after" is strict.
    #[test]
    fn a_later_hearing_wins_in_either_write_order(a in hearing(), gap in 1u64..5, len in 0u64..5, b_tags in tags()) {
        let b = (a.1 + gap, a.1 + gap + len, b_tags);
        let ab = written(&[a.clone(), b.clone()]);
        let ba = written(&[b.clone(), a]);
        let expected: std::collections::BTreeSet<String> = b.2.iter().cloned().collect();
        prop_assert_eq!(tag_set(&ab), expected.clone());
        prop_assert_eq!(tag_set(&ba), expected);
    }

    /// Two hearings that overlap leave the same record in either write
    /// order: the tags of both, over the span of both.
    #[test]
    fn overlapping_hearings_commute(a in hearing(), b in hearing()) {
        prop_assume!(b.0 < a.1 && a.0 < b.1);
        let ab = written(&[a.clone(), b.clone()]).expect("written");
        let ba = written(&[b.clone(), a.clone()]).expect("written");
        prop_assert_eq!(tag_set(&Some(ab.clone())), tag_set(&Some(ba.clone())));
        prop_assert_eq!(ab.heard.start, a.0.min(b.0));
        prop_assert_eq!(ab.heard.at, a.1.max(b.1));
        prop_assert_eq!(ab.heard, ba.heard);
    }

    /// Whatever is written, the record's span never moves back: its end
    /// only goes up.
    #[test]
    fn the_records_end_never_moves_back(hearings in proptest::collection::vec(hearing(), 1..8)) {
        let mut last_end = 0u64;
        for n in 1..=hearings.len() {
            let r = written(&hearings[..n]).expect("written");
            prop_assert!(r.heard.at >= last_end);
            last_end = r.heard.at;
        }
    }

    /// A failed read writes nothing, and a record of another run is
    /// replaced by the new one as it is.
    #[test]
    fn a_failed_read_or_another_run_decides_without_the_spans(a in hearing(), b in hearing()) {
        let rec = |run: u64, h: &(u64, u64, Vec<String>)| crate::commands::listing_tags::Record {
            heard: crate::commands::listing_tags::Hearing { run, start: h.0, at: h.1 },
            tags: h.2.clone(),
        };
        prop_assert_eq!(crate::commands::listing_tags::next_record(Err(()), rec(1, &b)), None);
        prop_assert_eq!(
            crate::commands::listing_tags::next_record(Ok(Some(rec(1, &a))), rec(2, &b)),
            Some(rec(2, &b))
        );
    }
}
