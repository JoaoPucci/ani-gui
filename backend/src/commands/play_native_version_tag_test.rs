//! A version tag hides nothing from the title rule: a tagged listing
//! of a season the entry does not name is refused as its plain copy is,
//! while which hits are probed is decided as before.

use crate::commands::play_native::pick_candidate_titled;
use crate::error::AniError;
use crate::scraper::provider::BrowseHit;
use crate::scraper::table_provider::{listing_of, Asked, TableProvider};

const YEAR: u32 = 2020;

fn hit(slug: &str, title: &str) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
        jname: None,
    }
}

fn site(shows: &[(u64, u32)]) -> TableProvider {
    shows.iter().fold(TableProvider::new(), |t, (id, count)| {
        t.listing(*id, listing_of(*id, *count))
            .year(*id, Some(YEAR))
    })
}

#[tokio::test]
async fn a_tagged_later_season_alone_is_refused_by_title() {
    for tagged in [
        "Chained Soldier Season 2 [Uncensored]",
        "Chained Soldier 2 (Uncensored)",
        "Chained Soldier 2nd Season （ＵＮＣＥＮＳＯＲＥＤ）",
    ] {
        let client = site(&[(1, 12)]);
        let hits = [hit("chained-soldier-1", tagged)];
        for year in [Some(YEAR), None] {
            let err = pick_candidate_titled(
                &client,
                &hits,
                Some(12),
                "Chained Soldier",
                &["Chained Soldier"],
                year,
                Some("TV"),
            )
            .await
            .expect_err(tagged);
            assert!(
                matches!(err, AniError::Upstream { status: 404 }),
                "{tagged} {year:?}: {err:?}"
            );
        }
    }
}

#[tokio::test]
async fn the_refused_tagged_season_is_still_probed_as_before() {
    // Which hits are probed does not move: the tagged later season is
    // probed, as evidence now rather than as a candidate, and the
    // entry's own listing beside it is picked.
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit(
            "chained-soldier-season-2-1",
            "Chained Soldier Season 2 [Uncensored]",
        ),
        hit("chained-soldier-2", "Chained Soldier"),
    ];
    let p = pick_candidate_titled(
        &client,
        &hits,
        Some(12),
        "Chained Soldier",
        &["Chained Soldier"],
        Some(YEAR),
        Some("TV"),
    )
    .await
    .expect("picked");
    assert_eq!(p.hit.slug, "chained-soldier-2");
    let probed: Vec<String> = client
        .asked()
        .into_iter()
        .filter_map(|a| match a {
            Asked::Episodes(slug) => Some(slug),
            _ => None,
        })
        .collect();
    assert_eq!(probed, ["chained-soldier-season-2-1", "chained-soldier-2"]);
}

#[tokio::test]
async fn a_countless_pick_passes_over_a_tagged_later_season() {
    let client = site(&[(1, 12)]);
    let hits = [hit(
        "chained-soldier-1",
        "Chained Soldier Season 2 [Uncensored]",
    )];
    let err = pick_candidate_titled(
        &client,
        &hits,
        None,
        "Chained Soldier",
        &["Chained Soldier"],
        Some(YEAR),
        Some("TV"),
    )
    .await
    .expect_err("not the entry");
    assert!(matches!(err, AniError::Upstream { status: 404 }), "{err:?}");
}

#[tokio::test]
async fn a_tag_after_the_entrys_own_season_still_admits_it() {
    let client = site(&[(1, 12)]);
    let hits = [hit(
        "chained-soldier-season-2-1",
        "Chained Soldier Season 2 [Uncensored]",
    )];
    let p = pick_candidate_titled(
        &client,
        &hits,
        Some(12),
        "Chained Soldier Season 2",
        &["Chained Soldier Season 2"],
        Some(YEAR),
        Some("TV"),
    )
    .await
    .expect("picked");
    assert_eq!(p.hit.slug, "chained-soldier-season-2-1");
}

#[tokio::test]
async fn a_tagged_entry_before_is_another_entry() {
    // "Show 1 [Uncensored]" is "Show 1" under a version tag: the entry
    // before "Show 2", never picked, and no refusal by title — the pool
    // is the clean miss it would be without it.
    for tagged in [
        "Show 1 [Uncensored]",
        "Show (Uncensored)",
        "Show Season 1 (Uncensored)",
    ] {
        let client = site(&[(1, 12)]);
        let hits = [hit("show-1", tagged)];
        let err = pick_candidate_titled(
            &client,
            &hits,
            Some(12),
            "Show 2",
            &["Show 2"],
            Some(YEAR),
            Some("TV"),
        )
        .await
        .expect_err(tagged);
        assert!(matches!(err, AniError::NoResults), "{tagged}: {err:?}");
    }
}

#[tokio::test]
async fn a_tag_hides_no_part_from_part_agreement() {
    // Both tagged cours are the entry's season; the one whose part
    // agrees — no part marker past the tag — is the pick, as it is for
    // the plain pair.
    for count in [Some(12), None] {
        let client = site(&[(1, 12), (2, 12)]);
        let hits = [
            hit(
                "show-season-2-part-2-1",
                "Show Season 2 Part 2 [Uncensored]",
            ),
            hit("show-season-2-2", "Show Season 2 [Uncensored]"),
        ];
        let p = pick_candidate_titled(
            &client,
            &hits,
            count,
            "Show Season 2",
            &["Show Season 2"],
            Some(YEAR),
            Some("TV"),
        )
        .await
        .expect("picked");
        assert_eq!(p.hit.slug, "show-season-2-2", "{count:?}");
    }
}

#[tokio::test]
async fn tagged_parts_stitch_as_their_plain_copies_do() {
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit("show-1", "Show [Uncensored]"),
        hit("show-part-2-2", "Show Part 2 [Uncensored]"),
    ];
    let p = pick_candidate_titled(
        &client,
        &hits,
        Some(24),
        "Show",
        &["Show"],
        Some(YEAR),
        Some("TV"),
    )
    .await
    .expect("picked");
    assert_eq!(p.hit.slug, "show-1");
    assert_eq!(p.episodes.len(), 24, "the two parts, stitched");
}

#[tokio::test]
async fn a_tagged_broad_listing_is_cut_as_its_plain_copy_is() {
    let client = site(&[(1, 22), (2, 10)]);
    let hits = [
        hit("show-season-3-1", "Show Season 3 (Uncensored)"),
        hit(
            "show-season-3-part-2-2",
            "Show Season 3 Part 2 (Uncensored)",
        ),
    ];
    let p = pick_candidate_titled(
        &client,
        &hits,
        Some(12),
        "Show Season 3",
        &["Show Season 3"],
        Some(YEAR),
        Some("TV"),
    )
    .await
    .expect("picked");
    assert_eq!(p.hit.slug, "show-season-3-1");
    assert_eq!(p.episodes.len(), 12, "cut to the entry's episodes");
}

#[tokio::test]
async fn parts_are_stitched_within_one_version() {
    // Both copies of both parts: the plain first part stitches to the
    // plain second part, whichever copy the provider lists first.
    for order in [[1u64, 2, 3, 4], [1, 2, 4, 3]] {
        let client = site(&[(1, 12), (2, 12), (3, 12), (4, 12)]);
        let all = [
            hit("show-1", "Show"),
            hit("show-2", "Show [Uncensored]"),
            hit("show-part-2-3", "Show Part 2 [Uncensored]"),
            hit("show-part-2-4", "Show Part 2"),
        ];
        let hits: Vec<_> = order
            .iter()
            .map(|i| all[(*i - 1) as usize].clone())
            .collect();
        let p = pick_candidate_titled(
            &client,
            &hits,
            Some(24),
            "Show",
            &["Show"],
            Some(YEAR),
            Some("TV"),
        )
        .await
        .expect("picked");
        assert_eq!(p.episodes.len(), 24, "{order:?}");
        let lead = p
            .hit
            .slug
            .rsplit('-')
            .next()
            .unwrap_or_default()
            .to_string();
        let second = p.episodes[12].id / 1000;
        let pair = (lead.as_str(), second);
        assert!(
            pair == ("1", 4) || pair == ("2", 3),
            "{order:?}: lead {} stitched to listing {second}",
            p.hit.slug
        );
    }
}
