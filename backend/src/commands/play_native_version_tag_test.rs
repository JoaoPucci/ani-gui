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
