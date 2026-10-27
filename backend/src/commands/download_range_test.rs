use super::episode_range;

#[test]
fn episode_range_accepts_ordered_integer_pairs_only() {
    // The UI sends "1-12" for All and "s-e" for Range; a start equal
    // to the end is normalized away by the UI but stays a valid pair.
    assert_eq!(episode_range("1-12"), Some((1, 12)));
    assert_eq!(episode_range("3-3"), Some((3, 3)));
    // Singles — integer and fractional — are the other path's job.
    assert_eq!(episode_range("5"), None);
    assert_eq!(episode_range("6.5"), None);
    // A reversed pair is not a range; it falls through to the
    // single-episode path, where it names no row and dead-ends as
    // the episode's own verdict, EpisodeUnavailable, rather than
    // silently downloading nothing.
    assert_eq!(episode_range("12-1"), None);
    // Fractional halves are not ranges either — the resolver's tag
    // match owns fractional identity.
    assert_eq!(episode_range("1.5-3"), None);
    assert_eq!(episode_range(""), None);
    assert_eq!(episode_range("-"), None);
}

mod props {
    use super::episode_range;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn every_ordered_pair_parses_and_nothing_else_does(a: u32, b: u32) {
            let joined = format!("{a}-{b}");
            let expected = (a <= b).then_some((a, b));
            prop_assert_eq!(episode_range(&joined), expected);
        }

        #[test]
        fn a_bare_integer_is_never_a_range(n: u32) {
            let single = n.to_string();
            prop_assert_eq!(episode_range(&single), None);
        }
    }
}

/// A range's start resolves against the version the download names
/// when the pick has several, as a single episode does.
#[tokio::test]
async fn a_range_starts_from_the_version_it_names() {
    use super::super::play_native_versions::VersionKind;
    use super::{Attempt, RangeStartAttempt};
    use crate::scraper::provider::BrowseHit;
    use crate::scraper::table_provider::{listing_of, TableProvider};
    let card = |slug: &str, title: &str| BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
        jname: None,
    };
    let provider = TableProvider::new()
        .search_default(vec![
            card("the-show-1", "The Show"),
            card("the-show-2", "The Show (Uncensored)"),
        ])
        .listing(1, listing_of(1, 3))
        .listing(2, listing_of(2, 3))
        .year(1, Some(2020))
        .year(2, Some(2020))
        .stream(1001, "sub", "https://cdn.example/plain/master.m3u8")
        .mode(1001, "sub", true)
        .stream(2001, "sub", "https://cdn.example/uncensored/master.m3u8")
        .mode(2001, "sub", true);
    for (version, slug) in [
        (Some(VersionKind::Standard), "the-show-1"),
        (Some(VersionKind::Uncensored), "the-show-2"),
        (None, "the-show-2"),
    ] {
        let args: super::DownloadArgs = serde_json::from_value(serde_json::json!({
            "title": "The Show", "episode": "1-3", "mode": "sub",
            "episode_count": 3, "year": 2020, "subtype": "TV",
        }))
        .expect("args");
        let args = super::DownloadArgs { version, ..args };
        let mut attempt = RangeStartAttempt {
            args: &args,
            first: 1,
            quality: "best",
            answered_by: None,
            tail: None,
            heard: None,
        };
        let (picked, served, _) = attempt.run(&provider).await.expect("started");
        assert_eq!(picked.hit.slug, slug, "{version:?}");
        assert_eq!(
            served.hit.slug, slug,
            "{version:?}: the version named served"
        );
    }
}

/// A range starts from a version that carries its mode when the one
/// it names does not.
#[tokio::test]
async fn a_range_in_a_mode_only_the_plain_copy_carries_starts_from_it() {
    use super::super::play_native_versions::VersionKind;
    use super::{Attempt, RangeStartAttempt};
    use crate::error::AniError;
    use crate::scraper::provider::BrowseHit;
    use crate::scraper::table_provider::{listing_of, Answer, TableProvider};
    let card = |slug: &str, title: &str| BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
        jname: None,
    };
    let provider = TableProvider::new()
        .search_default(vec![
            card("the-show-1", "The Show"),
            card("the-show-2", "The Show (Uncensored)"),
        ])
        .listing(1, listing_of(1, 3))
        .listing(2, listing_of(2, 3))
        .year(1, Some(2020))
        .year(2, Some(2020))
        .stream(1001, "dub", "https://cdn.example/plain/master.m3u8")
        .mode(1001, "dub", true)
        .stream_from(2001, "dub", Answer::fails(|| AniError::NoResults))
        .mode(2001, "dub", false);
    let args: super::DownloadArgs = serde_json::from_value(serde_json::json!({
        "title": "The Show", "episode": "1-3", "mode": "dub",
        "episode_count": 3, "year": 2020, "subtype": "TV",
    }))
    .expect("args");
    let args = super::DownloadArgs {
        version: Some(VersionKind::Uncensored),
        ..args
    };
    let mut attempt = RangeStartAttempt {
        args: &args,
        first: 1,
        quality: "best",
        answered_by: None,
        tail: None,
        heard: None,
    };
    let (picked, served, first) = attempt.run(&provider).await.expect("started");
    // The first episode streams from the copy that carries the dub,
    // which the start names as the version that served it; the range
    // keeps the version it names for the episodes after it.
    assert_eq!(first.master_url, "https://cdn.example/plain/master.m3u8");
    assert_eq!(served.hit.slug, "the-show-1");
    assert_eq!(picked.hit.slug, "the-show-2");
}
