//! The hero's pick, case by case: the rule as it stood before the
//! airing preference (ported with the pick from the renderer), the
//! preference on, and the walk each pick returns.

use super::*;

const BANNER: &str = "https://s4.anilist.co/file/anilistcdn/media/anime/banner/1.jpg";
const COVER: &str = "https://media.kitsu.app/anime/1/cover_image/large.jpg";

fn show(id: &str) -> HeroShow {
    HeroShow {
        id: id.into(),
        title: format!("Show {id}"),
        ..HeroShow::default()
    }
}
fn banner(id: &str) -> HeroShow {
    HeroShow {
        anilist_banner: Some(BANNER.into()),
        ..show(id)
    }
}
fn cover(id: &str) -> HeroShow {
    HeroShow {
        cover_image: Some(CoverSlots {
            large: Some(COVER.into()),
            ..CoverSlots::default()
        }),
        ..show(id)
    }
}
fn bare(id: &str) -> HeroShow {
    show(id)
}
fn airing(mut s: HeroShow) -> HeroShow {
    s.status = Some("current".into());
    s
}
fn finished(mut s: HeroShow) -> HeroShow {
    s.status = Some("finished".into());
    s
}

fn off(shows: &[HeroShow]) -> Pick {
    pick(shows, HERO_ROTATION_COUNT, false)
}
fn on(shows: &[HeroShow]) -> Pick {
    pick(shows, HERO_ROTATION_COUNT, true)
}

#[test]
fn art_counts_an_anilist_banner_or_any_slot_of_a_kitsu_cover() {
    assert_eq!(art(&banner("a")), Art::Banner);
    assert_eq!(art(&cover("a")), Art::Cover);
    let slot = |s: CoverSlots| HeroShow {
        cover_image: Some(s),
        ..show("a")
    };
    assert_eq!(
        art(&slot(CoverSlots {
            original: Some(COVER.into()),
            ..CoverSlots::default()
        })),
        Art::Cover
    );
    assert_eq!(
        art(&slot(CoverSlots {
            small: Some(COVER.into()),
            ..CoverSlots::default()
        })),
        Art::Cover
    );
    let both = HeroShow {
        anilist_banner: Some(BANNER.into()),
        ..cover("a")
    };
    assert_eq!(art(&both), Art::Banner);
}

#[test]
fn art_does_not_count_a_show_with_neither_or_a_cover_with_every_slot_empty() {
    assert_eq!(art(&bare("a")), Art::None);
    let empty = HeroShow {
        cover_image: Some(CoverSlots::default()),
        ..show("a")
    };
    assert_eq!(art(&empty), Art::None);
    let blank = HeroShow {
        anilist_banner: Some(String::new()),
        cover_image: Some(CoverSlots {
            large: Some(String::new()),
            ..CoverSlots::default()
        }),
        ..show("a")
    };
    assert_eq!(art(&blank), Art::None);
}

#[test]
fn airing_is_kitsus_current_status_and_nothing_else() {
    assert!(is_airing(&airing(bare("a"))));
    for status in [
        None,
        Some("finished"),
        Some("upcoming"),
        Some("tba"),
        Some("Current"),
    ] {
        let s = HeroShow {
            status: status.map(str::to_string),
            ..show("a")
        };
        assert!(!is_airing(&s), "{status:?}");
    }
}

#[test]
fn takes_the_first_three_shows_with_art_in_trending_order() {
    let t = [
        bare("1"),
        banner("2"),
        bare("3"),
        cover("4"),
        cover("5"),
        banner("6"),
    ];
    assert_eq!(off(&t).hero, ["2", "4", "5"]);
}

#[test]
fn leads_the_row_with_the_shows_it_skipped_then_the_rest_in_trending_order() {
    let t = [
        bare("1"),
        banner("2"),
        bare("3"),
        cover("4"),
        cover("5"),
        banner("6"),
        bare("7"),
    ];
    assert_eq!(off(&t).row, ["1", "3", "6", "7"]);
}

#[test]
fn is_the_plain_top_three_when_every_show_has_art() {
    let p = off(&[cover("1"), banner("2"), cover("3"), cover("4")]);
    assert_eq!(p.hero, ["1", "2", "3"]);
    assert_eq!(p.row, ["4"]);
}

#[test]
fn fills_the_rest_with_the_first_shows_without_art_when_fewer_than_three_have_it() {
    let p = off(&[bare("1"), bare("2"), cover("3"), bare("4"), bare("5")]);
    assert_eq!(p.hero, ["3", "1", "2"]);
    assert_eq!(p.row, ["4", "5"]);
}

#[test]
fn takes_every_show_when_there_are_fewer_than_three_art_first() {
    let p = off(&[bare("1"), cover("2")]);
    assert_eq!(p.hero, ["2", "1"]);
    assert!(p.row.is_empty());
}

#[test]
fn is_empty_for_an_empty_list() {
    let p = off(&[]);
    assert!(p.hero.is_empty() && p.row.is_empty() && p.walk.steps.is_empty());
}

#[test]
fn rotates_through_three_shows() {
    assert_eq!(HERO_ROTATION_COUNT, 3);
}

#[test]
fn takes_a_show_listed_twice_in_the_first_three_once() {
    let p = off(&[cover("1"), banner("1"), cover("2"), cover("3"), bare("4")]);
    assert_eq!(p.hero, ["1", "2", "3"]);
    assert_eq!(p.row, ["4"]);
}

#[test]
fn leaves_out_of_the_row_a_show_the_hero_already_took() {
    let p = off(&[
        cover("1"),
        cover("2"),
        cover("3"),
        bare("4"),
        cover("2"),
        bare("5"),
    ]);
    assert_eq!(p.hero, ["1", "2", "3"]);
    assert_eq!(p.row, ["4", "5"]);
}

#[test]
fn lists_a_show_repeated_in_the_row_once_in_its_first_place() {
    let p = off(&[
        cover("1"),
        cover("2"),
        cover("3"),
        bare("4"),
        bare("5"),
        bare("4"),
    ]);
    assert_eq!(p.row, ["4", "5"]);
}

#[test]
fn keeps_one_show_when_every_entry_is_the_same_show() {
    let p = off(&[cover("1"), cover("1"), banner("1")]);
    assert_eq!(p.hero, ["1"]);
    assert!(p.row.is_empty());
}

#[test]
fn keeps_the_first_entry_of_a_repeated_show_even_when_a_later_one_has_art() {
    let p = off(&[bare("1"), cover("2"), cover("1"), cover("3"), cover("4")]);
    assert_eq!(p.hero, ["2", "3", "4"]);
    assert_eq!(p.row, ["1"]);
}

#[test]
fn with_the_preference_on_airing_shows_with_art_lead_the_hero() {
    let t = [
        finished(cover("1")),
        finished(cover("2")),
        airing(banner("3")),
        finished(cover("4")),
        airing(cover("5")),
    ];
    let p = on(&t);
    assert_eq!(p.hero, ["3", "5", "1"]);
    assert_eq!(p.row, ["2", "4"]);
    assert_eq!(off(&t).hero, ["1", "2", "3"]);
}

#[test]
fn with_the_preference_on_an_airing_show_without_art_is_still_fill() {
    let t = [
        airing(bare("1")),
        finished(cover("2")),
        finished(cover("3")),
        finished(cover("4")),
    ];
    assert_eq!(on(&t).hero, ["2", "3", "4"]);
    assert_eq!(on(&t).row, ["1"]);
}

#[test]
fn with_the_preference_on_the_fill_keeps_trending_order() {
    let t = [bare("1"), airing(bare("2")), airing(cover("3")), bare("4")];
    assert_eq!(on(&t).hero, ["3", "1", "2"]);
}

#[test]
fn with_nothing_airing_the_preference_changes_nothing() {
    let t = [
        bare("1"),
        cover("2"),
        finished(banner("3")),
        cover("4"),
        cover("5"),
    ];
    assert_eq!(on(&t).hero, off(&t).hero);
    assert_eq!(on(&t).row, off(&t).row);
}

#[test]
fn the_walk_stops_at_the_show_that_settled_the_hero() {
    let t = [cover("1"), bare("2"), cover("3"), cover("4"), cover("5")];
    let w = off(&t).walk;
    assert_eq!(w.entries, 5);
    let ranks: Vec<usize> = w.steps.iter().map(|s| s.rank).collect();
    assert_eq!(ranks, [1, 2, 3, 4]);
    assert_eq!(
        w.steps[1].verdict,
        Verdict::Skipped {
            because: SkippedBecause::NoArt
        }
    );
    assert_eq!(
        w.steps[3].verdict,
        Verdict::Taken {
            slot: 2,
            because: TakenBecause::Art
        }
    );
}

#[test]
fn the_walk_says_which_shows_the_preference_lifted_and_which_it_passed_over() {
    let t = [
        finished(cover("1")),
        finished(cover("2")),
        finished(cover("3")),
        airing(cover("4")),
        finished(bare("5")),
    ];
    let w = on(&t).walk;
    assert!(w.airing_first);
    // Fewer than three airing shows with art: the whole list is walked.
    assert_eq!(w.steps.len(), 5);
    let verdicts: Vec<Verdict> = w.steps.iter().map(|s| s.verdict).collect();
    assert_eq!(
        verdicts,
        [
            Verdict::Taken {
                slot: 1,
                because: TakenBecause::Art
            },
            Verdict::Taken {
                slot: 2,
                because: TakenBecause::Art
            },
            Verdict::Skipped {
                because: SkippedBecause::NotAiring
            },
            Verdict::Taken {
                slot: 0,
                because: TakenBecause::AiringArt
            },
            Verdict::Skipped {
                because: SkippedBecause::NoArt
            },
        ]
    );
    let applied: Vec<bool> = w.steps.iter().map(|s| s.preference_applied).collect();
    assert_eq!(applied, [false, false, true, true, false]);
    assert_eq!(w.steps[3].art, Art::Cover);
    assert!(w.steps[3].airing);
    assert_eq!(w.steps[3].status.as_deref(), Some("current"));
    assert_eq!(w.steps[3].title, "Show 4");
}

#[test]
fn the_walk_names_repeats_and_a_full_hero() {
    let t = [cover("1"), cover("1"), cover("2"), cover("3")];
    let w = off(&t).walk;
    assert_eq!(
        w.steps[1].verdict,
        Verdict::Skipped {
            because: SkippedBecause::Repeat
        }
    );
    let p = on(&[
        airing(cover("1")),
        airing(cover("2")),
        cover("3"),
        airing(cover("4")),
        airing(cover("5")),
    ]);
    assert_eq!(p.hero, ["1", "2", "4"]);
    assert_eq!(
        p.walk.steps[2].verdict,
        Verdict::Skipped {
            because: SkippedBecause::NotAiring
        }
    );
    assert_eq!(p.walk.steps.len(), 4);
    let full = on(&[cover("1"), cover("2"), cover("3"), cover("4")]).walk;
    assert_eq!(
        full.steps[3].verdict,
        Verdict::Skipped {
            because: SkippedBecause::HeroFull
        }
    );
}

#[test]
fn an_airing_show_the_plain_order_takes_in_the_same_slot_was_not_decided_by_the_preference() {
    let p = on(&[airing(cover("1")), finished(cover("2")), airing(cover("3"))]);
    assert_eq!(p.hero, ["1", "3", "2"]);
    let steps = &p.walk.steps;
    assert_eq!(
        steps[0].verdict,
        Verdict::Taken {
            slot: 0,
            because: TakenBecause::Art
        }
    );
    assert!(!steps[0].preference_applied);
    // The third show moved up a slot because it airs.
    assert_eq!(
        steps[2].verdict,
        Verdict::Taken {
            slot: 1,
            because: TakenBecause::AiringArt
        }
    );
    assert!(steps[2].preference_applied);
}
