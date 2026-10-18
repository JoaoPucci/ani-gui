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
    // Both rules fire: the entry is shared with another mapped Kitsu
    // entry AND followed by an unmapped Kitsu part, each summing to it.
    let mut ev = tower_of_god("46312", 13);
    ev.sharing
        .push(kitsu("55555", Some(13), "2024-10-06", "2024-12-29"));
    let map = map_show(&ev);
    assert_eq!(map, identity(Identity::Conflict));
    assert!(!map.conclusive());
}
