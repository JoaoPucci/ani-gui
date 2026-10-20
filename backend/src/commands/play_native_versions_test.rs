//! Versions of one pick, case by case: the copies of a season the
//! picker reports as versions, and every shape it leaves as one pick.

use super::*;
use crate::commands::play_native::{pick_candidate_titled, PickedShow};
use crate::error::AniError;
use crate::scraper::table_provider::{Answer, TableProvider};

const YEAR: u32 = 2020;

fn hit(slug: &str, title: &str) -> BrowseHit {
    BrowseHit {
        slug: slug.into(),
        title: title.into(),
        kind: Some("TV".into()),
        jname: None,
    }
}

fn badged(slug: &str, title: &str, kind: &str) -> BrowseHit {
    BrowseHit {
        kind: Some(kind.into()),
        ..hit(slug, title)
    }
}

/// Every listing from the entry's year, `count` episodes each.
fn site(shows: &[(u64, u32)]) -> TableProvider {
    shows.iter().fold(TableProvider::new(), |t, (id, count)| {
        t.listing(*id, crate::scraper::table_provider::listing_of(*id, *count))
            .year(*id, Some(YEAR))
    })
}

/// The pick for a dated "The Show" of `count` episodes over `hits`.
async fn pick(
    client: &TableProvider,
    hits: &[BrowseHit],
    count: Option<u32>,
    titles: &[&str],
) -> crate::error::Result<PickedShow> {
    pick_candidate_titled(
        client,
        hits,
        count,
        titles[0],
        titles,
        Some(YEAR),
        Some("TV"),
    )
    .await
}

/// The versions a pick reports, as `(kind, slug)` in the order given.
fn versions(p: &PickedShow) -> Vec<(VersionKind, &str)> {
    p.versions
        .iter()
        .map(|v| (v.kind, v.hit.slug.as_str()))
        .collect()
}

#[tokio::test]
async fn both_copies_of_the_season_are_its_versions_and_uncensored_plays() {
    let client = site(&[(1, 12), (2, 12)]);
    // The plain copy is the exact title and first in the provider's
    // order: today's pick. The tagged copy is its other version.
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show (Uncensored)"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    assert_eq!(
        p.hit.slug, "the-show-2",
        "the uncensored copy is the default"
    );
    assert_eq!(
        p.episodes,
        crate::scraper::table_provider::listing_of(2, 12)
    );
    assert_eq!(
        versions(&p),
        vec![
            (VersionKind::Standard, "the-show-1"),
            (VersionKind::Uncensored, "the-show-2"),
        ]
    );
}

#[tokio::test]
async fn the_tag_is_read_in_any_case_and_bracket() {
    for tagged in [
        "The Show [Uncensored]",
        "The Show [UNCENSORED]",
        "The Show (uncensored)",
        "The Show （Uncensored）",
        "The Show【Uncensored】",
    ] {
        let client = site(&[(1, 12), (2, 12)]);
        let hits = [hit("the-show-1", "The Show"), hit("the-show-2", tagged)];
        let p = pick(&client, &hits, Some(12), &["The Show"])
            .await
            .expect("picked");
        assert_eq!(p.hit.slug, "the-show-2", "{tagged}");
        assert_eq!(p.versions.len(), 2, "{tagged}");
    }
}

#[tokio::test]
async fn the_word_outside_brackets_is_no_tag() {
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show Uncensored"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-1");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn a_tag_on_a_season_marker_still_names_the_same_season() {
    // The tag follows the marker it would hide; the plain copy says
    // which season it is, and both are the entry's.
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit("the-show-2nd-season-1", "The Show 2nd Season"),
        hit("the-show-2nd-season-2", "The Show 2nd Season (Uncensored)"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show 2nd Season"])
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-2nd-season-2");
    assert_eq!(p.versions.len(), 2);
}

#[tokio::test]
async fn a_copy_with_another_count_is_no_version() {
    // Within the tolerance and at another distance, or at the same
    // distance from the other side: either way not the same listing.
    for (plain, tagged) in [(12, 11), (11, 13)] {
        let client = site(&[(1, plain), (2, tagged)]);
        let hits = [
            hit("the-show-1", "The Show"),
            hit("the-show-2", "The Show (Uncensored)"),
        ];
        let p = pick(&client, &hits, Some(12), &["The Show"])
            .await
            .expect("picked");
        assert!(p.versions.is_empty(), "{plain} beside {tagged}");
        assert_eq!(
            p.hit.slug, "the-show-1",
            "{plain} beside {tagged}: today's pick"
        );
    }
}

#[tokio::test]
async fn a_copy_numbered_otherwise_is_no_version() {
    let client = site(&[(1, 12)]).listing(
        2,
        (1..=12)
            .map(|n| crate::scraper::provider::EpisodeRef {
                id: 2000 + u64::from(n),
                number: n + 12,
                number2: None,
                title: None,
                former_tag: None,
            })
            .collect::<Vec<_>>(),
    );
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show (Uncensored)"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn a_lone_copy_is_one_pick() {
    for title in ["The Show", "The Show (Uncensored)"] {
        let client = site(&[(1, 12)]);
        let hits = [hit("the-show-1", title)];
        let p = pick(&client, &hits, Some(12), &["The Show"])
            .await
            .expect("picked");
        assert_eq!(p.hit.slug, "the-show-1");
        assert!(p.versions.is_empty(), "{title}");
    }
}

#[tokio::test]
async fn another_show_under_the_tag_is_no_version() {
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-other-2", "The Show Other (Uncensored)"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-1");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn a_copy_under_another_badge_is_no_version() {
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit("the-show-1", "The Show"),
        badged("the-show-2", "The Show (Uncensored)", "ONA"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-1");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn two_copies_of_one_kind_are_no_versions() {
    // Which plain copy the tagged one pairs with cannot be told.
    let client = site(&[(1, 12), (2, 12), (3, 12)]);
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show (Uncensored)"),
        hit("the-show-3", "The Show"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-1");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn a_copy_the_year_excludes_is_no_version() {
    let client = site(&[(1, 12)])
        .listing(2, crate::scraper::table_provider::listing_of(2, 12))
        .year(2, Some(YEAR - 5));
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show (Uncensored)"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-1");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn a_tagged_copy_whose_plain_copy_is_refused_stays_one_pick() {
    // "The Show Season 2" names a season the entry does not; the tag
    // hides that marker from the title rule, so the tagged copy alone
    // is admitted — as it is today. Its plain copy cannot be picked,
    // so the two are not versions of a pick.
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit("the-show-season-2-1", "The Show Season 2"),
        hit("the-show-season-2-2", "The Show Season 2 (Uncensored)"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-season-2-2");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn a_dead_copy_leaves_the_pick_unheard() {
    // The tagged copy's probe dies: the versions cannot be told, and
    // playing the plain copy now and the tagged one later would split
    // the show's history across two keys on the strength of weather.
    let client = site(&[(1, 12)]).listing(2, Answer::fails(|| AniError::Network));
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show (Uncensored)"),
    ];
    let err = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect_err("a copy died unheard");
    assert!(matches!(err, AniError::Network), "{err:?}");
    // And the other way round.
    let client = site(&[(2, 12)]).listing(1, Answer::fails(|| AniError::Network));
    let err = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect_err("a copy died unheard");
    assert!(matches!(err, AniError::Network), "{err:?}");
}

#[tokio::test]
async fn a_countless_pick_has_no_versions() {
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show (Uncensored)"),
    ];
    let p = pick(&client, &hits, None, &["The Show"])
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-1");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn an_airing_rescue_has_no_versions() {
    // Both copies short of the count: the rescue takes one, as today.
    let client = site(&[(1, 4), (2, 4)]);
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show (Uncensored)"),
    ];
    let p = pick(&client, &hits, Some(24), &["The Show"])
        .await
        .expect("picked");
    assert_eq!(p.hit.slug, "the-show-1");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn choosing_a_version_plays_that_copy() {
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit("the-show-1", "The Show"),
        hit("the-show-2", "The Show (Uncensored)"),
    ];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    let standard = p.clone().choose(Some(VersionKind::Standard));
    assert_eq!(standard.hit.slug, "the-show-1");
    assert_eq!(
        standard.episodes,
        crate::scraper::table_provider::listing_of(1, 12)
    );
    assert_eq!(standard.versions, p.versions, "the versions travel with it");
    assert_eq!(p.clone().choose(None).hit.slug, "the-show-2");
    assert_eq!(
        p.clone().choose(Some(VersionKind::Uncensored)).hit.slug,
        "the-show-2"
    );
}

#[tokio::test]
async fn choosing_a_version_a_pick_lacks_keeps_the_pick() {
    let client = site(&[(1, 12)]);
    let hits = [hit("the-show-1", "The Show")];
    let p = pick(&client, &hits, Some(12), &["The Show"])
        .await
        .expect("picked");
    assert_eq!(
        p.clone().choose(Some(VersionKind::Uncensored)),
        p,
        "a show with one version plays it whatever is asked"
    );
}

#[test]
fn a_version_is_named_on_the_wire_in_lowercase() {
    assert_eq!(
        serde_json::to_string(&VersionKind::Uncensored).expect("json"),
        "\"uncensored\""
    );
    assert_eq!(
        serde_json::from_str::<VersionKind>("\"standard\"").expect("json"),
        VersionKind::Standard
    );
}

#[tokio::test]
async fn copies_whose_page_years_differ_within_the_filter_are_versions() {
    // The plain copy's page names the entry's year, the tagged one's
    // one off or none: both survive the year filter, and the year
    // tells no season from another.
    for tagged_year in [Some(YEAR - 1), None] {
        let client = site(&[(1, 12)])
            .listing(2, crate::scraper::table_provider::listing_of(2, 12))
            .year(2, tagged_year);
        let hits = [
            hit("the-show-1", "The Show"),
            hit("the-show-2", "The Show (Uncensored)"),
        ];
        let p = pick(&client, &hits, Some(12), &["The Show"])
            .await
            .expect("picked");
        assert_eq!(p.hit.slug, "the-show-2", "{tagged_year:?}");
        assert_eq!(p.versions.len(), 2, "{tagged_year:?}");
    }
}

#[tokio::test]
async fn an_undated_entry_scores_out_the_tagged_copy() {
    // With a count and no year only a name the entry carries vouches,
    // and the tag's words follow the name: the tagged copy is not the
    // entry's, and the plain copy is one pick.
    let client = site(&[(1, 12), (2, 12)]);
    let hits = [
        hit("the-show-2", "The Show (Uncensored)"),
        hit("the-show-1", "The Show"),
    ];
    let p = pick_candidate_titled(
        &client,
        &hits,
        Some(12),
        "The Show",
        &["The Show"],
        None,
        Some("TV"),
    )
    .await
    .expect("picked");
    assert_eq!(p.hit.slug, "the-show-1");
    assert!(p.versions.is_empty());
}

#[tokio::test]
async fn a_dead_copy_no_rule_would_pick_blocks_nothing() {
    // With a count and no year, or one video long and no year, the
    // tagged copy is named by none of the entry's titles and could
    // never be picked: its probe dying leaves the plain copy the pick,
    // as it is when the copy answers.
    for count in [12, 1] {
        let client = site(&[(1, count)]).listing(2, Answer::fails(|| AniError::Network));
        let hits = [
            hit("the-show-1", "The Show"),
            hit("the-show-2", "The Show (Uncensored)"),
        ];
        let p = pick_candidate_titled(
            &client,
            &hits,
            Some(count),
            "The Show",
            &["The Show"],
            None,
            Some("TV"),
        )
        .await
        .unwrap_or_else(|e| panic!("{count}: {e:?}"));
        assert_eq!(p.hit.slug, "the-show-1", "{count}");
        assert!(p.versions.is_empty(), "{count}");
    }
}
