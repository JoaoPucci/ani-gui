//! The rules that file a Kitsu episode under a tracker's entry, one
//! shape at a time, each against the show that showed it: Ranma ½ on
//! AniList for a chain of entries; Attack on Titan's Final Chapters and
//! Fei Ren Zai 2 for Kitsu entries sharing one MyAnimeList entry; Tower
//! of God's second season for Kitsu parts filed under one AniList entry;
//! Unlimited Blade Works for a prologue the tracker counts.

use super::*;
use crate::account::episode_map::{
    EarlierSegment, EpisodeMap, Evidence, Identity, KitsuShow, MappedWrite, Segment, Shape, Target,
    TrackerShow,
};

fn kitsu(id: &str, episodes: Option<u32>, start: &str, end: &str) -> KitsuShow {
    KitsuShow {
        id: id.into(),
        episodes,
        start: (!start.is_empty()).then(|| start.into()),
        end: (!end.is_empty()).then(|| end.into()),
    }
}

fn show(id: u32, episodes: Option<u32>, finished: bool, start: &str, end: &str) -> TrackerShow {
    TrackerShow {
        id,
        id_mal: None,
        episodes,
        finished,
        start: (!start.is_empty()).then(|| start.into()),
        end: (!end.is_empty()).then(|| end.into()),
    }
}

fn mapped(entry: u32, progress: u32, complete: bool, earlier: &[(u32, u32)]) -> Target {
    Target::Mapped(MappedWrite {
        entry,
        progress,
        complete,
        earlier: earlier
            .iter()
            .map(|&(entry, total)| EarlierSegment {
                entry,
                total,
                finished: true,
            })
            .collect(),
    })
}

fn identity(why: Identity) -> EpisodeMap {
    EpisodeMap::identity(why)
}

// --- a chain of the tracker's entries (Ranma ½ on AniList) --------------

/// Kitsu 186: 161 episodes, 1989-04-15 → 1992-09-25. AniList 210 is 18
/// of them (1989-04-15 → 1989-09-16); its sequel 149939, Nettou-hen, the
/// other 143 (1989-10-20 → 1992-09-25).
fn ranma() -> Evidence {
    Evidence {
        kitsu: kitsu("186", Some(161), "1989-04-15", "1992-09-25"),
        entry: show(210, Some(18), true, "1989-04-15", "1989-09-16"),
        chain: vec![show(149_939, Some(143), true, "1989-10-20", "1992-09-25")],
        ..Default::default()
    }
}

#[test]
fn a_chain_of_sequels_summing_to_kitsu_s_count_files_the_show_across_them() {
    let map = map_show(&ranma());
    assert_eq!(map.shape, Shape::Chain);
    assert_eq!(
        map.segments,
        vec![
            Segment {
                entry: 210,
                entry_total: 18,
                entry_finished: true,
                kitsu_from: 1,
                kitsu_to: 18,
                lead: 0,
                extras_after: vec![],
            },
            Segment {
                entry: 149_939,
                entry_total: 143,
                entry_finished: true,
                kitsu_from: 19,
                kitsu_to: 161,
                lead: 0,
                extras_after: vec![],
            },
        ]
    );
    assert!(map.conclusive());
}

#[test]
fn a_chain_s_episodes_are_sent_to_their_own_entry_under_their_own_number() {
    let map = map_show(&ranma());
    assert_eq!(map.target(1), mapped(210, 1, false, &[]));
    assert_eq!(map.target(17), mapped(210, 17, false, &[]));
    // Kitsu's 18 is the first entry's last episode: it completes it,
    // though the app is nowhere near its own finale.
    assert_eq!(map.target(18), mapped(210, 18, true, &[]));
    // Kitsu's 19 is Nettou-hen's first, and the entry before it has to
    // be complete.
    assert_eq!(map.target(19), mapped(149_939, 1, false, &[(210, 18)]));
    assert_eq!(map.target(100), mapped(149_939, 82, false, &[(210, 18)]));
    assert_eq!(map.target(161), mapped(149_939, 143, true, &[(210, 18)]));
}

#[test]
fn an_episode_past_kitsu_s_count_is_outside_every_segment() {
    assert_eq!(map_show(&ranma()).target(162), Target::Identity);
    assert_eq!(map_show(&ranma()).target(0), Target::Identity);
}

#[test]
fn a_chain_may_run_through_several_sequels() {
    // Mirumo de Pon on AniList: 78 + 24 + 48 + 22 = Kitsu's 172.
    let ev = Evidence {
        kitsu: kitsu("1564", Some(172), "2002-04-06", "2005-09-27"),
        entry: show(1744, Some(78), true, "2002-04-06", "2003-09-27"),
        chain: vec![
            show(170_426, Some(24), true, "2003-10-07", "2004-03-30"),
            show(170_427, Some(48), true, "2004-04-06", "2005-04-05"),
            show(170_428, Some(22), true, "2005-04-19", "2005-09-27"),
        ],
        ..Default::default()
    };
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::Chain);
    assert_eq!(
        map.segments
            .iter()
            .map(|s| (s.entry, s.kitsu_from, s.kitsu_to))
            .collect::<Vec<_>>(),
        [
            (1744, 1, 78),
            (170_426, 79, 102),
            (170_427, 103, 150),
            (170_428, 151, 172)
        ]
    );
    assert_eq!(
        map.target(151),
        mapped(
            170_428,
            1,
            false,
            &[(1744, 78), (170_426, 24), (170_427, 48)]
        )
    );
}

#[test]
fn a_chain_stops_at_the_sum_and_ignores_what_follows() {
    // Nettou-hen's own sequel (the OVA) is in the chain as the fetcher
    // followed it; the sum is reached before it and it is not a segment.
    let mut ev = ranma();
    ev.chain
        .push(show(1007, Some(6), true, "1993-10-21", "1994-02-18"));
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::Chain);
    assert_eq!(map.segments.len(), 2);
}

#[test]
fn a_chain_whose_sum_misses_kitsu_s_count_explains_nothing() {
    let mut over = ranma();
    over.chain[0].episodes = Some(144);
    assert_eq!(map_show(&over), identity(Identity::Unexplained));
    let mut under = ranma();
    under.chain[0].episodes = Some(142);
    assert_eq!(map_show(&under), identity(Identity::Unexplained));
    let mut unknown = ranma();
    unknown.chain[0].episodes = None;
    assert_eq!(map_show(&unknown), identity(Identity::Unexplained));
    let mut none = ranma();
    none.chain.clear();
    assert_eq!(map_show(&none), identity(Identity::Unexplained));
}

#[test]
fn a_chain_a_later_segment_of_which_another_kitsu_entry_claims_explains_nothing() {
    // To LOVE-Ru Darkness 2nd: 12 + a 2-episode special = Kitsu's 14,
    // but the special is a Kitsu entry of its own, mapped to it.
    let ev = Evidence {
        kitsu: kitsu("10078", Some(14), "2015-07-07", "2015-10-29"),
        entry: show(20_995, Some(12), true, "2015-07-07", "2015-09-29"),
        chain: vec![show(21_852, Some(2), true, "2015-10-29", "2015-10-29")],
        claimed: vec![21_852],
        ..Default::default()
    };
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
}

#[test]
fn a_chain_outside_kitsu_s_run_explains_nothing() {
    // Sonic X: 52 + 26 = Kitsu's 78, but Kitsu dates the show
    // 2003-04-06 → 2004-03-28 and the sequel ran 2005-09 → 2006-05.
    let ev = Evidence {
        kitsu: kitsu("1633", Some(78), "2003-04-06", "2004-03-28"),
        entry: show(1816, Some(52), true, "2003-04-06", "2004-03-28"),
        chain: vec![show(129_610, Some(26), true, "2005-09-10", "2006-05-06")],
        ..Default::default()
    };
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
    // A segment starting before Kitsu's run is as much outside it.
    let mut early = ranma();
    early.entry.start = Some("1989-03-01".into());
    assert_eq!(map_show(&early), identity(Identity::Unexplained));
}

/// A tracker's sequel that aired before the entry it follows started
/// is no later part of the show, whatever its relation says: both runs
/// lying inside Kitsu's does not place them in the chain's order.
/// Order is read only where the dates settle it — a month apart, or
/// whole days apart within one month — so a sequel dated by month alone
/// in its entry's month is read as it was.
#[test]
fn a_sequel_that_aired_before_its_entry_started_is_no_chain() {
    let ev = |entry_start: &str, sequel_start: &str| Evidence {
        kitsu: kitsu("1", Some(12), "2010-01-05", "2010-12-20"),
        entry: show(10, Some(6), true, entry_start, "2010-08-10"),
        chain: vec![show(11, Some(6), true, sequel_start, "2010-09-20")],
        ..Default::default()
    };
    assert_eq!(
        map_show(&ev("2010-07-01", "2010-01-10")),
        identity(Identity::Unexplained),
        "months apart"
    );
    assert_eq!(
        map_show(&ev("2010-07-20", "2010-07-03")),
        identity(Identity::Unexplained),
        "days apart in one month"
    );
    assert_eq!(
        map_show(&ev("2010-07-01", "2010-08-01")).shape,
        Shape::Chain
    );
    assert_eq!(map_show(&ev("2010-07", "2010-07")).shape, Shape::Chain);
    assert_eq!(
        map_show(&ev("2010-07-03", "2010-07-03")).shape,
        Shape::Chain
    );
}

#[test]
fn the_run_is_compared_by_month_so_a_day_s_disagreement_does_not_refuse_a_chain() {
    let mut ev = ranma();
    ev.kitsu.start = Some("1989-04-20".into());
    ev.chain[0].end = Some("1992-09-30".into());
    assert_eq!(map_show(&ev).shape, Shape::Chain);
}

#[test]
fn a_chain_needs_every_date_it_compares() {
    let mut no_kitsu_start = ranma();
    no_kitsu_start.kitsu.start = None;
    assert_eq!(map_show(&no_kitsu_start), identity(Identity::Unexplained));
    let mut no_kitsu_end = ranma();
    no_kitsu_end.kitsu.end = None;
    assert_eq!(map_show(&no_kitsu_end), identity(Identity::Unexplained));
    let mut no_segment_end = ranma();
    no_segment_end.chain[0].end = None;
    assert_eq!(map_show(&no_segment_end), identity(Identity::Unexplained));
    let mut year_only = ranma();
    year_only.entry.start = Some("1989".into());
    assert_eq!(map_show(&year_only), identity(Identity::Unexplained));
}

#[test]
fn a_segment_the_tracker_has_not_finished_is_not_completed_at_its_last_episode() {
    let mut ev = ranma();
    ev.chain[0].finished = false;
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::Chain);
    assert_eq!(map.target(161), mapped(149_939, 143, false, &[(210, 18)]));
    // The first segment's completion is its own.
    assert_eq!(map.target(18), mapped(210, 18, true, &[]));
}

#[test]
fn an_unfinished_earlier_segment_is_brought_to_its_total_without_completing() {
    let mut ev = ranma();
    ev.entry.finished = false;
    let map = map_show(&ev);
    assert_eq!(
        map.target(19),
        Target::Mapped(MappedWrite {
            entry: 149_939,
            progress: 1,
            complete: false,
            earlier: vec![EarlierSegment {
                entry: 210,
                total: 18,
                finished: false,
            }],
        })
    );
}

// --- identity -------------------------------------------------------------

#[test]
fn agreeing_counts_send_every_episode_as_it_is_conclusively() {
    let ev = Evidence {
        kitsu: kitsu("186", Some(161), "1989-04-15", "1992-09-25"),
        entry: show(210, Some(161), true, "1989-04-15", "1992-09-25"),
        ..Default::default()
    };
    let map = map_show(&ev);
    assert_eq!(map, identity(Identity::CountsAgree));
    assert!(map.conclusive());
    assert_eq!(map.target(1), Target::Identity);
    assert_eq!(map.target(161), Target::Identity);
}

#[test]
fn an_unknown_count_on_either_side_is_identity_for_now() {
    let mut ev = ranma();
    ev.kitsu.episodes = None;
    let map = map_show(&ev);
    assert_eq!(map, identity(Identity::CountUnknown));
    assert!(!map.conclusive());
    let mut ev = ranma();
    ev.entry.episodes = Some(0);
    assert_eq!(map_show(&ev), identity(Identity::CountUnknown));
}

#[test]
fn a_difference_nothing_explains_is_identity_for_now() {
    // Haruhi 2009 on AniList: 28 against Kitsu's 14, the surplus being
    // 2006 episodes the tracker's list interleaves.
    let ev = Evidence {
        kitsu: kitsu("3579", Some(14), "2009-05-22", "2009-09-11"),
        entry: show(4382, Some(28), true, "2009-04-03", "2009-10-09"),
        listed: Some((1..=28).map(|n| n.to_string()).collect()),
        sharing: vec![kitsu("3579", Some(14), "2009-05-22", "2009-09-11")],
        ..Default::default()
    };
    let map = map_show(&ev);
    assert_eq!(map, identity(Identity::Unexplained));
    assert!(!map.conclusive());
}

// --- Kitsu entries sharing one tracker entry (Attack on Titan's Final
// Chapters, Fei Ren Zai 2 on MyAnimeList) ---------------------------------

/// MAL 51535 is both Final Chapters specials, 2 episodes; Kitsu files
/// them as 46038 (2023-03-03) and 47132 (2023-11-04), one episode each,
/// both mapped to 51535.
fn final_chapters(ours: &str) -> Evidence {
    Evidence {
        kitsu: kitsu(ours, Some(1), "", ""),
        entry: show(51_535, Some(2), true, "2023-03-04", "2023-11-05"),
        sharing: vec![
            kitsu("47132", Some(1), "2023-11-04", "2023-11-04"),
            kitsu("46038", Some(1), "2023-03-03", "2023-03-03"),
        ],
        ..Default::default()
    }
}

#[test]
fn kitsu_entries_sharing_an_entry_take_it_in_date_order() {
    let first = map_show(&final_chapters("46038"));
    assert_eq!(first.shape, Shape::SharedEntry);
    assert_eq!(
        first.segments,
        vec![Segment {
            entry: 51_535,
            entry_total: 2,
            entry_finished: true,
            kitsu_from: 1,
            kitsu_to: 1,
            lead: 0,
            extras_after: vec![],
        }]
    );
    // The first part's last episode is not the entry's: progress alone.
    assert_eq!(first.target(1), mapped(51_535, 1, false, &[]));
    let second = map_show(&final_chapters("47132"));
    assert_eq!(second.shape, Shape::SharedEntry);
    assert_eq!(second.segments[0].lead, 1);
    // The last part's last episode is the entry's: it completes it.
    assert_eq!(second.target(1), mapped(51_535, 2, true, &[]));
}

#[test]
fn a_shared_entry_s_parts_may_differ_in_length() {
    // Fei Ren Zai 2 on MyAnimeList: 52523 is 48 episodes; Kitsu's
    // 50507 (12), 50508 (12) and 50509 (24) all map to it.
    let ev = Evidence {
        kitsu: kitsu("50509", Some(24), "2024-09-30", "2024-10-22"),
        entry: show(52_523, Some(48), true, "2022-08-10", "2024-10-22"),
        sharing: vec![
            kitsu("50507", Some(12), "2022-08-10", "2022-10-12"),
            kitsu("50508", Some(12), "2023-01-18", "2023-03-22"),
            kitsu("50509", Some(24), "2024-09-30", "2024-10-22"),
        ],
        ..Default::default()
    };
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::SharedEntry);
    assert_eq!(map.target(1), mapped(52_523, 25, false, &[]));
    assert_eq!(map.target(24), mapped(52_523, 48, true, &[]));
}

#[test]
fn a_shared_entry_whose_parts_do_not_sum_to_it_explains_nothing() {
    // Yamato 2202's OVAs beside the full series: three Kitsu entries
    // mapped to one 26-episode entry, 2 + 2 + 26.
    let ev = Evidence {
        kitsu: kitsu("12017", Some(2), "2017-02-25", "2017-02-25"),
        entry: show(33_064, Some(26), true, "2017-02-25", "2019-03-01"),
        sharing: vec![
            kitsu("12017", Some(2), "2017-02-25", "2017-02-25"),
            kitsu("41287", Some(2), "2017-02-25", "2019-03-01"),
            kitsu("43641", Some(26), "2017-02-25", "2019-03-01"),
        ],
        ..Default::default()
    };
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
}

#[test]
fn a_shared_entry_is_explained_only_by_every_part_mapped_to_it() {
    // Three distinctly dated twelve-episode Kitsu entries mapped to one
    // finished 24-episode entry: the first two alone sum to it, but the
    // three together are 36, so the set does not explain the entry and
    // the second's finale must not complete it.
    let sharing = vec![
        kitsu("1", Some(12), "2020-01-05", "2020-03-29"),
        kitsu("2", Some(12), "2020-07-05", "2020-09-27"),
        kitsu("3", Some(12), "2021-01-10", "2021-04-04"),
    ];
    for ours in ["1", "2", "3"] {
        let ev = Evidence {
            kitsu: sharing
                .iter()
                .find(|k| k.id == ours)
                .cloned()
                .expect("a part"),
            entry: show(700, Some(24), true, "2020-01-05", "2021-04-04"),
            sharing: sharing.clone(),
            ..Default::default()
        };
        assert_eq!(
            map_show(&ev),
            identity(Identity::Unexplained),
            "part {ours}"
        );
    }
}

#[test]
fn parts_that_reach_the_entry_explain_nothing_beside_a_later_part_mapped_to_it() {
    // The same three entries as one chain of sequels, all mapped to the
    // entry: the counts reach the entry's 24 at the second, but a part
    // mapped to the entry is not a sequel running on past it into entries
    // of its own, so the parts that reach the total are not the whole of
    // the entry.
    let sharing = vec![
        kitsu("1", Some(12), "2020-01-05", "2020-03-29"),
        kitsu("2", Some(12), "2020-07-05", "2020-09-27"),
        kitsu("3", Some(12), "2021-01-10", "2021-04-04"),
    ];
    for ours in ["1", "2"] {
        let ev = Evidence {
            kitsu: sharing
                .iter()
                .find(|k| k.id == ours)
                .cloned()
                .expect("a part"),
            entry: show(700, Some(24), true, "2020-01-05", "2021-04-04"),
            sharing: sharing.clone(),
            parts: sharing.clone(),
            ..Default::default()
        };
        assert_eq!(
            map_show(&ev),
            identity(Identity::Unexplained),
            "part {ours}"
        );
    }
}

#[test]
fn parts_that_reach_the_entry_explain_nothing_beside_another_entry_mapped_to_it() {
    // A chain of two twelve-episode parts reaches a 24-episode entry,
    // but a third twelve-episode Kitsu entry, outside the chain, is
    // mapped to the entry too: the mapped counts are 36, so neither
    // reading explains the entry.
    let a = kitsu("1", Some(12), "2020-01-05", "2020-03-29");
    let b = kitsu("2", Some(12), "2020-07-05", "2020-09-27");
    let outside = kitsu("9", Some(12), "2021-01-10", "2021-04-04");
    for ours in [&a, &b] {
        let ev = Evidence {
            kitsu: ours.clone(),
            entry: show(700, Some(24), true, "2020-01-05", "2021-04-04"),
            sharing: vec![a.clone(), b.clone(), outside.clone()],
            parts: vec![a.clone(), b.clone()],
            ..Default::default()
        };
        assert_eq!(
            map_show(&ev),
            identity(Identity::Unexplained),
            "part {}",
            ours.id
        );
    }
    // Where Kitsu's page of the entries mapped to the tracker's entry did
    // not hold them all, the parts are not known to be the whole of it.
    let c = kitsu("3", Some(12), "2021-01-10", "2021-04-04");
    let ev = Evidence {
        kitsu: b.clone(),
        entry: show(700, Some(24), true, "2020-01-05", "2021-04-04"),
        sharing: vec![KitsuShow::default()],
        parts: vec![a.clone(), b.clone(), c],
        ..Default::default()
    };
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
}

#[test]
fn a_shared_set_and_a_chain_that_file_the_entry_differently_are_a_conflict() {
    // A chain of three twelve-episode parts against a 24-episode entry:
    // Kitsu maps the first and third to the entry, and the second to
    // nothing. The mapped set sums to the entry with the third as its
    // 13th–24th, but the chain reaches the entry's total with the second
    // as those episodes. The two file the entry differently, so neither
    // explains it, whichever part is the show.
    let a = kitsu("1", Some(12), "2020-01-05", "2020-03-29");
    let b = kitsu("2", Some(12), "2020-07-05", "2020-09-27");
    let c = kitsu("3", Some(12), "2021-01-10", "2021-04-04");
    for ours in [&a, &c] {
        let ev = Evidence {
            kitsu: ours.clone(),
            entry: show(700, Some(24), true, "2020-01-05", "2021-04-04"),
            sharing: vec![a.clone(), c.clone()],
            parts: vec![a.clone(), b.clone()],
            ..Default::default()
        };
        assert_eq!(
            map_show(&ev),
            identity(Identity::Conflict),
            "part {}",
            ours.id
        );
    }
}

#[test]
fn a_shared_entry_needs_distinct_known_start_dates_to_order_its_parts() {
    let mut same_day = final_chapters("46038");
    same_day.sharing[0].start = Some("2023-03-03".into());
    assert_eq!(map_show(&same_day), identity(Identity::Unexplained));
    let mut undated = final_chapters("46038");
    undated.sharing[1].start = None;
    assert_eq!(map_show(&undated), identity(Identity::Unexplained));
}

#[test]
fn a_shared_entry_s_parts_must_include_the_show_being_mapped() {
    let mut ev = final_chapters("46038");
    ev.sharing[1].id = "99999".into();
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
    let mut alone = final_chapters("46038");
    alone.sharing.truncate(1);
    alone.sharing[0].id = "46038".into();
    assert_eq!(map_show(&alone), identity(Identity::Unexplained));
}

// --- Kitsu parts filed under one entry, the later ones with no entry of
// their own on the tracker (Tower of God's second season on AniList) ----

/// AniList 153406 is Tower of God season 2, 26 episodes. Kitsu files
/// it as 46312 (13) and its sequel 49244, Koubou-sen (13), which has a
/// MyAnimeList id AniList does not know, so no AniList entry.
fn tower_of_god(ours: &str, ours_count: u32) -> Evidence {
    Evidence {
        kitsu: kitsu(ours, Some(ours_count), "", ""),
        entry: show(153_406, Some(26), true, "2024-07-07", "2024-12-29"),
        sharing: vec![kitsu("46312", Some(13), "2024-07-07", "2024-09-29")],
        parts: vec![
            kitsu("46312", Some(13), "2024-07-07", "2024-09-29"),
            kitsu("49244", Some(13), "2024-10-06", "2024-12-29"),
        ],
        ..Default::default()
    }
}

#[test]
fn kitsu_parts_filed_under_one_entry_take_it_in_order() {
    let first = map_show(&tower_of_god("46312", 13));
    assert_eq!(first.shape, Shape::SplitParts);
    assert_eq!(first.segments[0].lead, 0);
    assert_eq!(first.target(13), mapped(153_406, 13, false, &[]));
    let second = map_show(&tower_of_god("49244", 13));
    assert_eq!(second.shape, Shape::SplitParts);
    assert_eq!(
        second.segments,
        vec![Segment {
            entry: 153_406,
            entry_total: 26,
            entry_finished: true,
            kitsu_from: 1,
            kitsu_to: 13,
            lead: 13,
            extras_after: vec![],
        }]
    );
    assert_eq!(second.target(1), mapped(153_406, 14, false, &[]));
    assert_eq!(second.target(13), mapped(153_406, 26, true, &[]));
}

#[test]
fn the_parts_are_read_as_far_as_the_sum_and_no_further() {
    // PriPara on MyAnimeList: 23135 is 140 episodes; Kitsu's 8555 (38),
    // 10209 (51) and 12683 (51) are filed under it, and 12683's own
    // sequel follows in the chain the fetcher walked.
    let ev = Evidence {
        kitsu: kitsu("12683", Some(51), "", ""),
        entry: show(23_135, Some(140), true, "2014-07-05", "2017-03-28"),
        parts: vec![
            kitsu("8555", Some(38), "2014-07-05", "2015-03-31"),
            kitsu("10209", Some(51), "2015-04-07", "2016-03-29"),
            kitsu("12683", Some(51), "2016-04-05", "2017-03-28"),
            kitsu("13000", Some(51), "2017-04-04", "2018-03-27"),
        ],
        ..Default::default()
    };
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::SplitParts);
    assert_eq!(map.segments[0].lead, 89);
    assert_eq!(map.target(51), mapped(23_135, 140, true, &[]));
    // A part past the sum is not under the entry.
    let mut past = ev.clone();
    past.kitsu = kitsu("13000", Some(51), "", "");
    assert_eq!(map_show(&past), identity(Identity::Unexplained));
}

#[test]
fn parts_that_do_not_sum_to_the_entry_explain_nothing() {
    let mut ev = tower_of_god("49244", 13);
    ev.parts[1].episodes = Some(12);
    ev.kitsu.episodes = Some(12);
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
    let mut unknown = tower_of_god("46312", 13);
    unknown.parts[1].episodes = None;
    assert_eq!(map_show(&unknown), identity(Identity::Unexplained));
    let mut alone = tower_of_god("46312", 13);
    alone.parts.truncate(1);
    assert_eq!(map_show(&alone), identity(Identity::Unexplained));
}

#[test]
fn two_explanations_of_one_difference_are_no_explanation() {
    // Both rules fire and disagree: the entry is shared with another
    // mapped Kitsu entry dated before ours (ours its 14th–26th), AND
    // ours is followed by an unmapped Kitsu part (ours its 1st–13th).
    let mut ev = tower_of_god("46312", 13);
    ev.sharing
        .push(kitsu("55555", Some(13), "2024-01-07", "2024-03-29"));
    let map = map_show(&ev);
    assert_eq!(map, identity(Identity::Conflict));
    assert!(!map.conclusive());
}

// --- episodes the tracker counts that Kitsu does not list (Unlimited
// Blade Works on AniList) ---------------------------------------------------

fn listed(numbers: &[&str]) -> Option<Vec<String>> {
    Some(numbers.iter().map(|n| (*n).to_string()).collect())
}

/// AniList 19603 counts 13 episodes of Unlimited Blade Works' first
/// season, its list opening on "Episode 0 - Prologue"; Kitsu's 7882
/// numbers 12. (The list also runs on through the second season.)
fn ubw() -> Evidence {
    let mut numbers: Vec<String> = vec!["0".into()];
    numbers.extend((1..=25).map(|n| n.to_string()));
    Evidence {
        kitsu: kitsu("7882", Some(12), "2014-10-12", "2014-12-28"),
        entry: show(19_603, Some(13), true, "2014-10-05", "2014-12-28"),
        listed: Some(numbers),
        sharing: vec![kitsu("7882", Some(12), "2014-10-12", "2014-12-28")],
        ..Default::default()
    }
}

#[test]
fn a_prologue_the_tracker_counts_shifts_every_episode_by_one() {
    let map = map_show(&ubw());
    assert_eq!(map.shape, Shape::Extras);
    assert_eq!(
        map.segments,
        vec![Segment {
            entry: 19_603,
            entry_total: 13,
            entry_finished: true,
            kitsu_from: 1,
            kitsu_to: 12,
            lead: 0,
            extras_after: vec![0],
        }]
    );
    assert_eq!(map.target(1), mapped(19_603, 2, false, &[]));
    assert_eq!(map.target(11), mapped(19_603, 12, false, &[]));
    // Kitsu's last episode is the entry's: it sends the total, complete.
    assert_eq!(map.target(12), mapped(19_603, 13, true, &[]));
}

#[test]
fn several_recaps_the_tracker_counts_shift_the_episodes_after_each() {
    // A long show whose tracker counts a prologue and three half
    // episodes: 24 of Kitsu's against 28.
    let mut numbers: Vec<String> = (1..=24).map(|n| n.to_string()).collect();
    numbers.extend(["0", "6.5", "12.5", "18.5"].map(String::from));
    let ev = Evidence {
        kitsu: kitsu("1", Some(24), "2020-01-05", "2020-06-28"),
        entry: show(2, Some(28), true, "2020-01-05", "2020-06-28"),
        listed: Some(numbers),
        ..Default::default()
    };
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::Extras);
    assert_eq!(map.segments[0].extras_after, vec![0, 6, 12, 18]);
    assert_eq!(map.target(1), mapped(2, 2, false, &[]));
    assert_eq!(map.target(6), mapped(2, 7, false, &[]));
    assert_eq!(map.target(7), mapped(2, 9, false, &[]));
    assert_eq!(map.target(13), mapped(2, 16, false, &[]));
    assert_eq!(map.target(19), mapped(2, 23, false, &[]));
    assert_eq!(map.target(24), mapped(2, 28, true, &[]));
}

#[test]
fn an_extra_after_kitsu_s_last_episode_is_counted_by_that_episode() {
    let ev = Evidence {
        kitsu: kitsu("1", Some(12), "2020-01-05", "2020-03-28"),
        entry: show(2, Some(13), true, "2020-01-05", "2020-03-28"),
        listed: listed(&[
            "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "12.5",
        ]),
        ..Default::default()
    };
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::Extras);
    assert_eq!(map.target(11), mapped(2, 11, false, &[]));
    assert_eq!(map.target(12), mapped(2, 13, true, &[]));
}

#[test]
fn an_extra_listed_once_per_track_counts_once() {
    // AniList lists an episode's subbed and dubbed tracks separately.
    let mut ev = ubw();
    ev.listed = listed(&["0", "0", "1", "1", "2", "2"]);
    assert_eq!(map_show(&ev).shape, Shape::Extras);
}

#[test]
fn extras_that_do_not_account_for_the_whole_surplus_explain_nothing() {
    // Seikaisuru Kado: 13 against 12, but the list carries both an
    // "Episode 0" and a "6.5" — two extras for one surplus episode.
    let mut two_for_one = ubw();
    two_for_one.listed = listed(&["0", "1", "2", "3", "4", "5", "6", "6.5", "7"]);
    assert_eq!(map_show(&two_for_one), identity(Identity::Unexplained));
    // To Be Heroine: 12 against 7, one "4.5" listed.
    let mut one_for_five = ubw();
    one_for_five.entry.episodes = Some(17);
    one_for_five.listed = listed(&["1", "2", "3", "4", "4.5", "5"]);
    assert_eq!(map_show(&one_for_five), identity(Identity::Unexplained));
    // No list at all (MyAnimeList, or an entry AniList lists nothing
    // for): the surplus is unexplained.
    let mut unlisted = ubw();
    unlisted.listed = None;
    assert_eq!(map_show(&unlisted), identity(Identity::Unexplained));
    let mut whole_only = ubw();
    whole_only.listed = listed(&["1", "2", "3"]);
    assert_eq!(map_show(&whole_only), identity(Identity::Unexplained));
}

#[test]
fn an_extra_past_kitsu_s_count_is_not_inside_the_entry() {
    let mut ev = ubw();
    ev.listed = listed(&["1", "2", "3", "13.5"]);
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
}

#[test]
fn a_number_the_list_cannot_be_read_as_is_ignored() {
    let mut ev = ubw();
    ev.listed = listed(&["0", "1", "Prologue", "", "2.x", "3"]);
    assert_eq!(map_show(&ev).shape, Shape::Extras);
}

#[test]
fn extras_and_parts_explaining_the_same_surplus_conflict() {
    // Kamitsubaki on AniList: 13 against 12, with an "Episode 0" listed
    // and a one-episode Kitsu sequel with no AniList entry.
    let ev = Evidence {
        kitsu: kitsu("49900", Some(12), "2025-07-09", "2025-09-25"),
        entry: show(173_335, Some(13), true, "2025-07-03", "2025-09-25"),
        listed: listed(&["0", "1", "2", "3", "4"]),
        parts: vec![
            kitsu("49900", Some(12), "2025-07-09", "2025-09-25"),
            kitsu("50027", Some(1), "2025-10-02", "2025-10-02"),
        ],
        ..Default::default()
    };
    assert_eq!(map_show(&ev), identity(Identity::Conflict));
}

#[test]
fn two_rules_filing_the_episodes_the_same_way_are_one_explanation() {
    // The Final Chapters specials: both Kitsu entries map to MAL 51535
    // and Kitsu chains the second after the first, so the shared-entry
    // and split-parts rules each file the second at the entry's 2nd.
    let mut ev = final_chapters("47132");
    ev.parts = vec![
        kitsu("46038", Some(1), "2023-03-03", "2023-03-03"),
        kitsu("47132", Some(1), "2023-11-04", "2023-11-04"),
    ];
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::SharedEntry);
    assert_eq!(map.target(1), mapped(51_535, 2, true, &[]));
    // Filing them differently is still a conflict.
    ev.parts.reverse();
    assert_eq!(map_show(&ev), identity(Identity::Conflict));
}

#[test]
fn a_later_part_is_not_filed_where_its_first_part_s_map_would_conflict() {
    // Kamitsubaki's one-episode Kitsu sequel: from its own view only the
    // split-parts rule fires (13 = 12 + 1), but from the first part's
    // the entry's Episode 0 explains the same surplus — the two
    // disagree about where the sequel's episode is, so neither part is
    // filed.
    let ev = Evidence {
        kitsu: kitsu("50027", Some(1), "2025-10-02", "2025-10-02"),
        entry: show(173_335, Some(13), true, "2025-07-03", "2025-09-25"),
        listed: listed(&["0", "1", "2", "3", "4"]),
        parts: vec![
            kitsu("49900", Some(12), "2025-07-09", "2025-09-25"),
            kitsu("50027", Some(1), "2025-10-02", "2025-10-02"),
        ],
        ..Default::default()
    };
    assert_eq!(map_show(&ev), identity(Identity::Conflict));
    // With no Episode 0 listed, the sequel is the entry's 13th.
    let mut plain = ev.clone();
    plain.listed = listed(&["1", "2", "3"]);
    assert_eq!(map_show(&plain).target(1), mapped(173_335, 13, true, &[]));
}

#[test]
fn extras_are_read_only_for_an_entry_s_one_kitsu_entry() {
    // A second season with no AniList entry of its own, filed through
    // its prequel's: AniList's entry is 13 with an Episode 0, the first
    // season Kitsu's 12. The surplus is over the first season, not the
    // second — the second season's episodes are not the entry's.
    let s1 = kitsu("1", Some(12), "2020-01-05", "2020-03-29");
    let s2 = kitsu("2", Some(12), "2020-07-05", "2020-09-27");
    let unresolved = Evidence {
        kitsu: s2.clone(),
        entry: show(500, Some(13), true, "2019-12-29", "2020-03-29"),
        listed: listed(&["0", "1", "2"]),
        sharing: vec![s1.clone()],
        parts: vec![s1.clone(), s2.clone()],
        ..Default::default()
    };
    assert_eq!(map_show(&unresolved), identity(Identity::Unexplained));
    // Kitsu maps both seasons to the one entry: the same.
    let mut shared = unresolved.clone();
    shared.sharing = vec![s1.clone(), s2.clone()];
    shared.parts = vec![s2];
    assert_eq!(map_show(&shared), identity(Identity::Unexplained));
    // The first season alone under the entry is the extras rule's case.
    let alone = Evidence {
        kitsu: s1.clone(),
        sharing: vec![s1.clone()],
        parts: vec![s1],
        ..unresolved
    };
    assert_eq!(map_show(&alone).shape, Shape::Extras);
}

#[test]
fn distinct_fractions_after_one_episode_are_distinct_extras() {
    // "6.1" and "6.2" are two extras; with "0" that is three for a
    // surplus of two, so the list does not account for it.
    let mut ev = ubw();
    ev.entry.episodes = Some(14);
    ev.listed = listed(&["0", "1", "6.1", "6.2", "6.2"]);
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
    ev.entry.episodes = Some(15);
    let map = map_show(&ev);
    assert_eq!(map.segments[0].extras_after, vec![0, 6, 6]);
    assert_eq!(map.target(7), mapped(19_603, 10, false, &[]));
}

#[test]
fn a_kitsu_entry_the_mappings_page_did_not_list_keeps_the_list_from_explaining_alone() {
    // Kitsu maps more entries to the tracker's id than its page held:
    // the gatherer records one it did not list (no id, no count), so
    // the show is not known to be the entry's only Kitsu entry.
    let mut ev = ubw();
    ev.sharing = vec![
        kitsu("7882", Some(12), "2014-10-12", "2014-12-28"),
        KitsuShow::default(),
    ];
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
}

#[test]
fn a_number_written_with_trailing_zeros_is_the_same_number() {
    // "6.5" and "6.50" are one extra; "6.0" is episode 6.
    let mut ev = ubw();
    ev.entry.episodes = Some(14);
    ev.listed = listed(&["0", "1", "6.5", "6.50", "6.0", "7"]);
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::Extras);
    assert_eq!(map.segments[0].extras_after, vec![0, 6]);
}

// --- a chain Kitsu's dates leave out, which its own episodes date -----

fn airdates(pairs: &[(u32, &str)]) -> std::collections::BTreeMap<u32, String> {
    pairs.iter().map(|(n, d)| (*n, (*d).to_string())).collect()
}

/// Sonic X on AniList: 52 + 26 = Kitsu's 78, but Kitsu dates the show
/// to the Japanese broadcast (2003-04-06 → 2004-03-28) while the third
/// season ran 2005-09-10 → 2006-05-06.
fn sonic_x() -> Evidence {
    Evidence {
        kitsu: kitsu("1633", Some(78), "2003-04-06", "2004-03-28"),
        entry: show(1816, Some(52), true, "2003-04-06", "2004-03-28"),
        chain: vec![show(129_610, Some(26), true, "2005-09-10", "2006-05-06")],
        ..Default::default()
    }
}

#[test]
fn a_segment_outside_kitsu_s_run_is_admitted_where_kitsu_s_own_episodes_date_it() {
    let mut ev = sonic_x();
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
    // Kitsu's episode 53 aired 2005-09-10 and its 78th 2006-05-06: its
    // own list puts the third season where AniList does.
    ev.kitsu_airdates = airdates(&[
        (1, "2003-08-23"),
        (52, "2005-03-26"),
        (53, "2005-09-10"),
        (78, "2006-05-06"),
    ]);
    let map = map_show(&ev);
    assert_eq!(map.shape, Shape::Chain);
    assert_eq!(map.target(53), mapped(129_610, 1, false, &[(1816, 52)]));
}

#[test]
fn kitsu_s_dates_for_a_segment_must_meet_its_run_at_both_ends() {
    // The third season lies wholly past Kitsu's end, so both its bounds
    // need Kitsu's own dates. Only the start dated: the end is not
    // vouched for.
    let mut ev = sonic_x();
    ev.kitsu_airdates = airdates(&[(53, "2005-09-10")]);
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
    // Dated, but not in the month the segment's run ends.
    ev.kitsu_airdates = airdates(&[(53, "2005-09-10"), (78, "2006-06-06")]);
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
}

#[test]
fn a_first_segment_kitsu_dates_later_is_admitted_by_its_first_episode_s_date() {
    // Monsuno: AniList's first season premiered 2012-02-23 (Kitsu's own
    // episode 1 is dated that day); Kitsu's start date is 2012-10-03.
    let ev = Evidence {
        kitsu: kitsu("6907", Some(52), "2012-10-03", "2013-09-25"),
        entry: show(13_185, Some(26), true, "2012-02-23", "2012-11-21"),
        chain: vec![show(197_976, Some(26), true, "2013-04-03", "2013-09-25")],
        kitsu_airdates: airdates(&[(1, "2012-02-23")]),
        ..Default::default()
    };
    assert_eq!(map_show(&ev).shape, Shape::Chain);
    let mut undated = ev;
    undated.kitsu_airdates.clear();
    assert_eq!(map_show(&undated), identity(Identity::Unexplained));
}

#[test]
fn a_run_a_day_or_two_past_kitsu_s_across_a_month_is_inside_it() {
    // Youkoso Japari Park: Kitsu ends 2020-09-30, AniList's second
    // season 2020-10-01.
    let ev = Evidence {
        kitsu: kitsu("41933", Some(36), "2018-08-10", "2020-09-30"),
        entry: show(104_187, Some(24), true, "2018-08-12", "2019-10-15"),
        chain: vec![show(118_168, Some(12), true, "2020-04-15", "2020-10-01")],
        ..Default::default()
    };
    assert_eq!(map_show(&ev).shape, Shape::Chain);
    // Three days is not a day's disagreement.
    let mut late = ev.clone();
    late.chain[0].end = Some("2020-10-03".into());
    assert_eq!(map_show(&late), identity(Identity::Unexplained));
    // A year's (Ooya-san to Boku: Kitsu 2021, the trackers 2020) is not.
    let ooya = Evidence {
        kitsu: kitsu("44220", Some(10), "2021-03-02", "2021-06-12"),
        entry: show(117_280, Some(5), true, "2020-03-02", "2020-03-06"),
        chain: vec![show(149_039, Some(5), true, "2020-06-08", "2020-06-12")],
        ..Default::default()
    };
    assert_eq!(map_show(&ooya), identity(Identity::Unexplained));
}

#[test]
fn a_segment_past_kitsu_s_end_needs_its_start_dated_too() {
    // Sonic X's third season lies wholly after Kitsu's end: its start is
    // after Kitsu's start but also after Kitsu's end, so both of its
    // bounds need Kitsu's own dates.
    let mut ev = sonic_x();
    ev.kitsu_airdates = airdates(&[(78, "2006-05-06")]);
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
    // Kitsu's own list putting episode 53 in 2004 contradicts it.
    ev.kitsu_airdates = airdates(&[(53, "2004-01-10"), (78, "2006-05-06")]);
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
}

#[test]
fn a_segment_before_kitsu_s_start_needs_its_end_dated_too() {
    // A first season Kitsu dates later than the tracker does, ending
    // before Kitsu's start: the end needs Kitsu's date as well.
    let ev = Evidence {
        kitsu: kitsu("1", Some(24), "2013-01-05", "2013-06-29"),
        entry: show(10, Some(12), true, "2012-01-07", "2012-03-31"),
        chain: vec![show(11, Some(12), true, "2013-04-06", "2013-06-29")],
        kitsu_airdates: airdates(&[(1, "2012-01-07")]),
        ..Default::default()
    };
    assert_eq!(map_show(&ev), identity(Identity::Unexplained));
    let mut both = ev;
    both.kitsu_airdates = airdates(&[(1, "2012-01-07"), (12, "2012-03-31")]);
    assert_eq!(map_show(&both).shape, Shape::Chain);
}

#[test]
fn a_start_a_day_or_two_before_kitsu_s_is_inside_it() {
    let ev = Evidence {
        kitsu: kitsu("1", Some(24), "2015-01-01", "2015-06-30"),
        entry: show(10, Some(12), true, "2014-12-30", "2015-03-25"),
        chain: vec![show(11, Some(12), true, "2015-04-01", "2015-06-24")],
        ..Default::default()
    };
    assert_eq!(map_show(&ev).shape, Shape::Chain);
    let mut early = ev;
    early.entry.start = Some("2014-12-28".into());
    assert_eq!(map_show(&early), identity(Identity::Unexplained));
}

#[test]
fn the_kitsu_episodes_a_chain_needs_dated_are_its_unplaced_bounds() {
    // Sonic X: only the third season's end needs a date… and, as its
    // start is past Kitsu's end, its start too.
    assert_eq!(
        chain_dates_needed(&sonic_x()),
        std::collections::BTreeSet::from([53, 78])
    );
    // A chain inside Kitsu's run needs none; nor a claimed one, nor one
    // whose sum misses, nor one missing a date.
    assert!(chain_dates_needed(&ranma()).is_empty());
    let mut claimed = sonic_x();
    claimed.claimed = vec![129_610];
    assert!(chain_dates_needed(&claimed).is_empty());
    let mut short = sonic_x();
    short.chain[0].episodes = Some(25);
    assert!(chain_dates_needed(&short).is_empty());
    let mut undated = sonic_x();
    undated.chain[0].end = None;
    assert!(chain_dates_needed(&undated).is_empty());
    // A one-and-one chain is a chain too.
    let one_and_one = Evidence {
        kitsu: kitsu("1", Some(2), "2020-01-01", "2020-01-31"),
        entry: show(10, Some(1), true, "2020-01-01", "2020-01-01"),
        chain: vec![show(11, Some(1), true, "2021-05-01", "2021-05-01")],
        ..Default::default()
    };
    assert_eq!(
        chain_dates_needed(&one_and_one),
        std::collections::BTreeSet::from([2])
    );
}

#[test]
fn a_chain_whose_dates_name_no_real_day_or_month_is_refused() {
    for bad in [
        "1989-04-31",
        "1989-13-01",
        "1989-00-15",
        "1989-04-xx",
        "1989-13",
    ] {
        let mut ev = ranma();
        ev.entry.start = Some(bad.into());
        assert_eq!(map_show(&ev), identity(Identity::Unexplained), "{bad}");
    }
    // A month a source knows without its day is still a date.
    let mut ev = ranma();
    ev.entry.start = Some("1989-04".into());
    assert_eq!(map_show(&ev).shape, Shape::Chain);
}

#[test]
fn shared_parts_are_ordered_only_by_real_full_start_dates() {
    // A start short of a day, or naming no real day, cannot order the
    // parts: "2023-03" against "2023-03-03", "2023-3-3", "2023-02-31".
    for bad in ["2023-03", "2023-3-3", "2023-02-31", "2023-13-01"] {
        let mut ev = final_chapters("47132");
        ev.sharing[1].start = Some(bad.into());
        assert_eq!(map_show(&ev), identity(Identity::Unexplained), "{bad}");
    }
}
