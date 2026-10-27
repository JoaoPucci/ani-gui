//! The special rule against the shapes Kitsu's catalogue showed: the
//! eight one-episode prologues AniList has no entry for and counts as
//! the "Episode 0" of the related season's entry (Unlimited Blade Works
//! among them), Kamitsubaki's special whose dates settle it before the
//! series, Lupin Part 6's whose dates put it a day after, and every way
//! the evidence falls short.

use super::*;
use crate::account::episode_map::{Identity, Segment, Shape, Target};

fn kitsu(id: &str, episodes: Option<u32>, start: &str) -> KitsuShow {
    KitsuShow {
        id: id.into(),
        episodes,
        start: (!start.is_empty()).then(|| start.into()),
        end: (!start.is_empty()).then(|| start.into()),
    }
}

fn entry(id: u32, total: u32, start: &str, end: &str) -> TrackerShow {
    TrackerShow {
        id,
        id_mal: None,
        episodes: Some(total),
        finished: true,
        start: Some(start.into()),
        end: Some(end.into()),
    }
}

/// The map the extras rule gives a season of `count` Kitsu episodes on
/// an entry of `total`, its extras after the Kitsu episodes `after`.
fn extras_map(id: u32, total: u32, count: u32, after: &[u32]) -> EpisodeMap {
    EpisodeMap {
        shape: Shape::Extras,
        segments: vec![Segment {
            entry: id,
            entry_total: total,
            entry_finished: true,
            kitsu_from: 1,
            kitsu_to: count,
            lead: 0,
            extras_after: after.to_vec(),
        }],
    }
}

/// Unlimited Blade Works' prologue (Kitsu 9718, 2014-10-05) and the TV
/// season it is related to, 7882, whose AniList entry 19603 counts 13
/// against Kitsu's 12 with an "Episode 0" listed; and Fate/Zero's second
/// season, its prequel, on an AniList entry of its own whose counts
/// agree.
fn ubw() -> SpecialEvidence {
    SpecialEvidence {
        special: kitsu("9718", Some(1), "2014-10-05"),
        related: vec![
            RelatedFiling {
                related: kitsu("7658", Some(12), "2012-04-08"),
                entry: entry(11_741, 12, "2012-04-08", "2012-06-24"),
                map: EpisodeMap::identity(Identity::CountsAgree),
                contested: false,
            },
            RelatedFiling {
                related: kitsu("7882", Some(12), "2014-10-12"),
                entry: entry(19_603, 13, "2014-10-05", "2014-12-28"),
                map: extras_map(19_603, 13, 12, &[0]),
                contested: false,
            },
        ],
    }
}

fn written(map: &EpisodeMap, k: u32) -> Option<(u32, u32, bool)> {
    match map.target(k) {
        Target::Mapped(w) if w.earlier.is_empty() => Some((w.entry, w.progress, w.complete)),
        Target::Mapped(w) => panic!("a special's write brings up no entry first: {w:?}"),
        Target::Identity => None,
    }
}

#[test]
fn a_prologue_the_related_season_s_entry_counts_is_that_entry_s_first_episode() {
    let map = special_extras(&ubw()).expect("filed");
    assert_eq!(map.shape, Shape::SpecialExtras);
    assert_eq!(written(&map, 1), Some((19_603, 1, false)));
    assert_eq!(written(&map, 2), None, "past the special's count");
    assert!(map.conclusive());
}

/// Kamitsubaki's special (50027) aired on 2025-07-03, six days before
/// the series it relates as its prequel; the series' entry, 173335,
/// lists an Episode 0, and the series' own map reads it alone.
#[test]
fn kamitsubaki_s_special_is_its_series_entry_s_first_episode() {
    let ev = SpecialEvidence {
        special: kitsu("50027", Some(1), "2025-07-03"),
        related: vec![RelatedFiling {
            related: kitsu("49900", Some(12), "2025-07-09"),
            entry: entry(173_335, 13, "2025-07-03", "2025-09-25"),
            map: extras_map(173_335, 13, 12, &[0]),
            contested: false,
        }],
    };
    let map = special_extras(&ev).expect("filed");
    assert_eq!(written(&map, 1), Some((173_335, 1, false)));
}

/// Lupin Part 6's special (45318) aired on 2021-10-10, a day after the
/// series' start as Kitsu dates it. Its place is the one AniList lists
/// for the extra, not its date's: AniList lists it as Episode 0, so it
/// is the entry's first episode all the same.
#[test]
fn a_special_dated_after_its_series_takes_the_place_the_tracker_lists_for_it() {
    let ev = SpecialEvidence {
        special: kitsu("45318", Some(1), "2021-10-10"),
        related: vec![RelatedFiling {
            related: kitsu("44618", Some(24), "2021-10-09"),
            entry: entry(134_623, 25, "2021-10-10", "2022-03-26"),
            map: extras_map(134_623, 25, 24, &[0]),
            contested: false,
        }],
    };
    assert_eq!(
        written(&special_extras(&ev).expect("filed"), 1),
        Some((134_623, 1, false))
    );
    // A recap the entry lists as 6.5 is its seventh episode.
    let mut recap = ev.clone();
    recap.related[0].map = extras_map(134_623, 25, 24, &[6]);
    assert_eq!(
        written(&special_extras(&recap).expect("filed"), 1),
        Some((134_623, 7, false))
    );
}

#[test]
fn a_special_the_related_entry_s_map_does_not_read_as_extras_is_filed_nowhere() {
    for map in [
        EpisodeMap::identity(Identity::Conflict),
        EpisodeMap::identity(Identity::Unexplained),
        EpisodeMap::identity(Identity::CountsAgree),
        EpisodeMap {
            shape: Shape::SplitParts,
            ..extras_map(19_603, 13, 12, &[])
        },
    ] {
        let mut ev = ubw();
        ev.related[1].map = map.clone();
        assert_eq!(special_extras(&ev), None, "{map:?}");
    }
}

#[test]
fn a_special_whose_count_is_not_the_extras_count_is_filed_nowhere() {
    // Seikaisuru Kado's shape: two extras listed, a one-episode special.
    let mut two = ubw();
    two.related[1].map = extras_map(19_603, 14, 12, &[0, 6]);
    assert_eq!(special_extras(&two), None);
    let mut longer = ubw();
    longer.special.episodes = Some(2);
    assert_eq!(special_extras(&longer), None);
    for unknown in [None, Some(0)] {
        let mut ev = ubw();
        ev.special.episodes = unknown;
        assert_eq!(special_extras(&ev), None, "{unknown:?}");
    }
}

#[test]
fn several_episodes_take_consecutive_extras_or_none() {
    let mut ev = ubw();
    ev.special.episodes = Some(2);
    ev.related[1].map = extras_map(19_603, 14, 12, &[0, 0]);
    let map = special_extras(&ev).expect("filed");
    assert_eq!(written(&map, 1), Some((19_603, 1, false)));
    assert_eq!(written(&map, 2), Some((19_603, 2, false)));
    // Extras apart in the entry are no run the special's episodes fill.
    ev.related[1].map = extras_map(19_603, 14, 12, &[0, 6]);
    assert_eq!(special_extras(&ev), None);
}

#[test]
fn an_extra_that_is_the_entry_s_last_episode_is_filed_nowhere() {
    // Filed there, the special's one episode would complete the entry.
    let mut ev = ubw();
    ev.related[1].map = extras_map(19_603, 13, 12, &[12]);
    assert_eq!(special_extras(&ev), None);
}

#[test]
fn a_special_another_related_entry_could_be_is_filed_nowhere() {
    let mut ev = ubw();
    ev.related[1].contested = true;
    assert_eq!(special_extras(&ev), None);
}

#[test]
fn two_related_entries_whose_maps_both_could_hold_it_file_it_nowhere() {
    let mut ev = ubw();
    ev.related[0].map = extras_map(11_741, 13, 12, &[0]);
    ev.related[0].entry.episodes = Some(13);
    ev.related[0].entry.start = Some("2014-10-01".into());
    ev.related[0].entry.end = Some("2014-12-28".into());
    assert_eq!(special_extras(&ev), None);
}

#[test]
fn a_special_whose_date_does_not_lie_in_the_entry_s_run_is_filed_nowhere() {
    for start in [
        "",
        "2014-10",
        "2014",
        "2014-09-30",
        "2015-01-01",
        "2014-02-30",
    ] {
        let mut ev = ubw();
        ev.special.start = (!start.is_empty()).then(|| start.into());
        assert_eq!(special_extras(&ev), None, "{start:?}");
    }
    // The run is the entry's, compared by month: late in its last month
    // is inside.
    let mut late = ubw();
    late.special.start = Some("2014-12-31".into());
    assert!(special_extras(&late).is_some());
    // An entry whose own dates are not known places nothing.
    let mut undated = ubw();
    undated.related[1].entry.start = None;
    assert_eq!(special_extras(&undated), None);
}

#[test]
fn a_map_whose_segment_is_not_the_whole_season_from_the_entry_s_start_is_no_reading() {
    let mut led = ubw();
    led.related[1].map.segments[0].lead = 3;
    assert_eq!(special_extras(&led), None);
    let mut elsewhere = ubw();
    elsewhere.related[1].map.segments[0].entry = 20_792;
    assert_eq!(special_extras(&elsewhere), None);
}

#[test]
fn the_special_rule_and_the_others_together() {
    let filed = special_extras(&ubw()).expect("filed");
    let identity = EpisodeMap::identity(Identity::Unexplained);
    // Nothing else explains it: the special rule's map.
    assert_eq!(with_special(identity.clone(), Some(filed.clone())), filed);
    assert_eq!(
        with_special(
            EpisodeMap::identity(Identity::CountUnknown),
            Some(filed.clone())
        ),
        filed
    );
    // Nothing from the special rule: the others' map, as it was.
    assert_eq!(with_special(identity.clone(), None), identity);
    // A conflict stays one.
    let conflict = EpisodeMap::identity(Identity::Conflict);
    assert_eq!(
        with_special(conflict.clone(), Some(filed.clone())),
        conflict
    );
    // Another rule filing it the same way is one explanation; otherwise
    // two explanations are a conflict.
    let same = EpisodeMap {
        shape: Shape::SplitParts,
        segments: filed.segments.clone(),
    };
    assert_eq!(with_special(same.clone(), Some(filed.clone())), same);
    let other = EpisodeMap {
        shape: Shape::SplitParts,
        segments: vec![Segment {
            lead: 12,
            ..filed.segments[0].clone()
        }],
    };
    assert_eq!(with_special(other.clone(), Some(filed)), conflict);
    assert_eq!(with_special(other.clone(), None), other);
}
