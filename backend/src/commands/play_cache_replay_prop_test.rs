//! Properties of what a replay names: the former number it hands the
//! session, the listing tags it checks that number against, and the
//! window title its handoff opens with.
//!
//! Strategies are boxed so the Windows test build's debug info stays
//! under the linker's PDB limit.

use super::{cached_launch_args, cached_watch};
use crate::commands::play_native_merge::{carries_tag, former_on_replay};
use proptest::prelude::*;

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
        crate::commands::listing_tags::put(&state, &show_key, &recorded, 1);
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
    crate::commands::listing_tags::put(&state, key, &["24.5".to_string()], 200);
    crate::commands::listing_tags::put(&state, key, &[], 100);
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
    let newer = crate::commands::listing_tags::heard_now() + 60_000;
    for key in ["test-a:show-uncut-1", "test-a:show-2"] {
        crate::commands::listing_tags::put(&state, key, &["2.5".to_string()], newer);
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
    /// A hearing replaces a record exactly when nothing is recorded or it
    /// is no earlier than the hearing recorded.
    #[test]
    fn a_hearing_replaces_only_an_earlier_or_equal_one(
        stored in proptest::option::of(any::<u64>()),
        at in any::<u64>(),
    ) {
        let replaces = crate::commands::listing_tags::supersedes(stored, at);
        let expected = match stored {
            None => true,
            Some(s) => at >= s,
        };
        prop_assert_eq!(replaces, expected);
    }

    /// Whatever order a show's listings are recorded in, the record holds
    /// the one heard last; among those heard at the same moment, the one
    /// written last.
    #[test]
    fn the_record_holds_the_listing_heard_last_whatever_the_write_order(
        hearings in proptest::collection::vec((0u64..20, tags()), 1..8),
    ) {
        let state = crate::commands::play::tests::state_with_proxy_origin();
        for (at, recorded) in &hearings {
            crate::commands::listing_tags::put(&state, "test-a:show-1", recorded, *at);
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
