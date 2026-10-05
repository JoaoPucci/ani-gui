//! Tests for `crate::commands::play_native_split`.

use super::*;
use crate::commands::play_native_numbering::{extra_episode_tags, kitsu_episode_cap};

const SBR: &str = "Steel Ball Run: JoJo no Kimyou na Bouken";

fn row(id: u64, number: u32, tag: Option<&str>) -> EpisodeRef {
    EpisodeRef {
        id,
        number,
        number2: tag.map(str::to_string),
    }
}

fn listing(first_id: u64, count: u32) -> Vec<EpisodeRef> {
    (1..=count)
        .map(|n| row(first_id + u64::from(n), n, None))
        .collect()
}

fn cand(title: &str, count: u32, confirmed: bool) -> PartCandidate<'_> {
    PartCandidate {
        title,
        count,
        confirmed,
        offset: 0,
    }
}

#[test]
fn an_airing_split_chains_both_parts_in_order() {
    // The provider lists the 2nd Stage first; the chain still starts
    // at the stem.
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(&second, 2, true), cand(SBR, 1, true)];
    assert_eq!(split_chain(&cands, 12, 10, &[]), Some(vec![1, 0]));
}

#[test]
fn a_finished_split_beats_the_near_miss_single_part() {
    // 1 + 11 = 12 exactly, where the 2nd Stage alone is one off and
    // would otherwise win inside the tolerance.
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 11, true)];
    assert_eq!(split_chain(&cands, 12, 1, &[]), Some(vec![0, 1]));
}

#[test]
fn three_parts_chain_in_order() {
    let cands = [
        cand("The Show Part 3", 4, true),
        cand("The Show", 4, true),
        cand("The Show Part 2", 4, true),
    ];
    assert_eq!(split_chain(&cands, 12, 8, &[]), Some(vec![1, 2, 0]));
}

#[test]
fn no_chain_where_a_single_part_already_fits() {
    let cands = [
        cand("The Show", 12, true),
        cand("The Show 2nd Season", 1, true),
    ];
    assert_eq!(split_chain(&cands, 12, 0, &[]), None);
    // Still none when the chain only ties the single part.
    let cands = [
        cand("The Show", 11, true),
        cand("The Show 2nd Season", 2, true),
    ];
    assert_eq!(split_chain(&cands, 12, 1, &[]), None);
}

#[test]
fn no_chain_without_both_years_confirmed() {
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 2, false)];
    assert_eq!(split_chain(&cands, 12, 10, &[]), None);
    let cands = [cand(SBR, 1, false), cand(&second, 2, true)];
    assert_eq!(split_chain(&cands, 12, 10, &[]), None);
}

#[test]
fn a_sibling_with_nothing_listed_adds_nothing() {
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 0, true)];
    assert_eq!(split_chain(&cands, 12, 11, &[]), None);
}

#[test]
fn no_chain_past_the_expected_count_outside_the_tolerance() {
    let cands = [
        cand("The Show", 12, true),
        cand("The Show Part 2", 12, true),
    ];
    assert_eq!(split_chain(&cands, 12, 0, &[]), None);
    let cands = [
        cand("The Show", 10, true),
        cand("The Show Part 2", 10, true),
    ];
    assert_eq!(split_chain(&cands, 12, 2, &[]), None);
}

#[test]
fn a_gap_in_the_parts_ends_the_chain() {
    let cands = [cand("The Show", 4, true), cand("The Show Part 3", 4, true)];
    assert_eq!(split_chain(&cands, 12, 8, &[]), None);
}

#[test]
fn stitching_numbers_the_later_part_after_the_earlier_one() {
    // Steel Ball Run as hianime lists it.
    let first = vec![row(1216, 1, None)];
    let second = vec![row(149_865, 1, None), row(149_931, 2, None)];
    let merged = merge_parts(&[&first, &second]);
    let by_number: Vec<(u32, u64)> = merged.iter().map(|e| (e.number, e.id)).collect();
    assert_eq!(by_number, vec![(1, 1216), (2, 149_865), (3, 149_931)]);
    assert_eq!(kitsu_episode_cap(&merged), Some(3));
}

#[test]
fn stitching_normalises_a_cumulative_part_first() {
    // A continuation part numbered cumulatively (41, 42) still lands
    // straight after the part before it.
    let first = listing(100, 2);
    let second = vec![row(201, 41, None), row(202, 42, None)];
    let merged = merge_parts(&[&first, &second]);
    let numbers: Vec<u32> = merged.iter().map(|e| e.number).collect();
    assert_eq!(numbers, vec![1, 2, 3, 4]);
    assert_eq!(merged[2].id, 201);
}

#[test]
fn stitching_moves_a_recap_tag_with_its_part() {
    let first = listing(100, 2);
    let second = vec![
        row(201, 1, None),
        row(202, 2, Some("1.5")),
        row(203, 3, Some("2")),
    ];
    let merged = merge_parts(&[&first, &second]);
    assert_eq!(kitsu_episode_cap(&merged), Some(4));
    assert_eq!(extra_episode_tags(&merged), vec!["3.5".to_string()]);
    // The integer-tagged row is episode 4 of the entry.
    let four = merged
        .iter()
        .find(|e| e.number2.as_deref().map_or(e.number == 4, |t| t == "4"))
        .expect("episode 4");
    assert_eq!(four.id, 203);
}

#[test]
fn one_part_stitches_to_itself() {
    let only = listing(100, 3);
    assert_eq!(merge_parts(&[&only]), only);
}

proptest::proptest! {
    #[test]
    fn stitching_keeps_every_row_and_sums_the_caps(
        counts in proptest::collection::vec(1u32..30, 1..4),
    ) {
        let parts: Vec<Vec<EpisodeRef>> = counts
            .iter()
            .enumerate()
            .map(|(i, c)| listing((i as u64 + 1) * 1000, *c))
            .collect();
        let refs: Vec<&[EpisodeRef]> = parts.iter().map(Vec::as_slice).collect();
        let merged = merge_parts(&refs);
        proptest::prop_assert_eq!(merged.len(), parts.iter().map(Vec::len).sum::<usize>());
        proptest::prop_assert_eq!(kitsu_episode_cap(&merged), Some(counts.iter().sum::<u32>()));
        let numbers: Vec<u32> = merged.iter().map(|e| e.number).collect();
        let want: Vec<u32> = (1..=counts.iter().sum::<u32>()).collect();
        proptest::prop_assert_eq!(numbers, want);
    }
}

#[test]
fn stitched_slots_stay_unique_when_a_part_carries_a_recap() {
    // A part that lists a recap between two regular episodes: "3.5" in
    // slot 4, episode "4" in slot 5. Every merged row needs a slot of
    // its own — the slot is what history stores and what the display
    // stamp maps back to a tag, so two rows sharing one would show a
    // real episode as the recap on resume.
    let first = listing(100, 2);
    let second = vec![
        row(201, 1, None),
        row(202, 2, None),
        row(203, 3, None),
        row(204, 4, Some("3.5")),
        row(205, 5, Some("4")),
    ];
    let merged = merge_parts(&[&first, &second]);
    let mut slots: Vec<u32> = merged.iter().map(|e| e.number).collect();
    slots.sort_unstable();
    slots.dedup();
    assert_eq!(slots.len(), merged.len(), "slots {merged:?}");
    // Each row keeps its identity in the entry's numbering.
    let shown: Vec<String> = merged
        .iter()
        .map(|e| e.number2.clone().unwrap_or_else(|| e.number.to_string()))
        .collect();
    assert_eq!(shown, ["1", "2", "3", "4", "5", "5.5", "6"]);
    assert_eq!(kitsu_episode_cap(&merged), Some(6));
    assert_eq!(extra_episode_tags(&merged), vec!["5.5".to_string()]);
}

#[test]
fn stitched_slots_count_rows_like_a_single_listing_does() {
    // A single listing's slot is the row's position; a stitched one is
    // numbered the same way, across every part, so a slot read back
    // from history names one row however the listing was assembled.
    let first = vec![
        row(101, 1, None),
        row(102, 2, Some("1.5")),
        row(103, 3, Some("2")),
    ];
    let second = listing(200, 2);
    let merged = merge_parts(&[&first, &second]);
    let slots: Vec<u32> = merged.iter().map(|e| e.number).collect();
    assert_eq!(slots, [1, 2, 3, 4, 5]);
}

#[test]
fn a_bare_title_that_already_fits_is_not_stitched_to_its_sequel() {
    // Kitsu's cour 1 counts 13; the provider lists the bare title with
    // 12 and a same-year "Part 2" — its own Kitsu entry — that has
    // aired one. 12 + 1 hits 13 exactly, but the bare title alone is
    // inside the tolerance: it is the entry, and stitching would play
    // Part 2's first episode as episode 13.
    let cands = [cand("X", 12, true), cand("X Part 2", 1, true)];
    assert_eq!(split_chain(&cands, 13, 1, &[]), None);
}

#[test]
fn a_near_miss_that_is_a_later_part_is_stitched_whatever_the_titles_spell() {
    // The provider's titles need not read like Kitsu's: hianime spells
    // Steel Ball Run in English while Kitsu searches it in romaji. What
    // decides is which candidate is the near miss — here the 2nd Stage,
    // one off, a later part of the chain — not how the lead is spelled.
    let lead = "Steel Ball Run: JoJo's Bizarre Adventure";
    let second = format!("{lead} 2nd Stage");
    let cands = [cand(lead, 1, true), cand(&second, 11, true)];
    assert_eq!(split_chain(&cands, 12, 1, &[]), Some(vec![0, 1]));
}

#[test]
fn an_airing_split_stays_stitched_once_its_later_part_reaches_the_tolerance() {
    // Mid-season the 2nd Stage alone comes within the tolerance (9 of
    // 12 aired against an expected 12 less the premiere). Were the pick
    // to flip to it then, the show's key and every episode's number
    // would move under the user part-way through the season.
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 9, true)];
    assert_eq!(split_chain(&cands, 12, 3, &[]), Some(vec![0, 1]));
}

#[test]
fn a_lead_numbered_cumulatively_is_never_stitched() {
    // A lead listed as a continuation (41, 42, ...) was played alone
    // with that offset stamped under its key, and its history rows
    // speak those numbers. Stitching would restamp the key at zero
    // and misread every one of them.
    let mut lead = cand("The Show", 2, true);
    lead.offset = 40;
    let cands = [lead, cand("The Show Part 2", 2, true)];
    assert_eq!(split_chain(&cands, 12, 10, &[]), None);
}

/// The best single candidate's distance, as the picker computes it.
fn best_single(cands: &[PartCandidate<'_>], expected: u32) -> u32 {
    cands
        .iter()
        .map(|c| c.count.abs_diff(expected))
        .min()
        .unwrap_or(u32::MAX)
}

#[test]
fn steel_ball_run_as_hianime_lists_it_mid_season_is_stitched() {
    // hianime's pool for Kitsu's Steel Ball Run (12 episodes, 2026),
    // in search order: the March premiere alone, then the weekly run
    // with two episodes aired. Kitsu's episode 2 is the 2nd Stage's 1.
    let lead = "Steel Ball Run: JoJo's Bizarre Adventure";
    let second = "Steel Ball Run: JoJo's Bizarre Adventure 2nd Stage";
    let cands = [cand(lead, 1, true), cand(second, 2, true)];
    assert_eq!(
        split_chain(&cands, 12, best_single(&cands, 12), &[SBR]),
        Some(vec![0, 1])
    );
}

#[test]
fn same_year_sequels_hianime_lists_beside_their_first_cour_are_not_stitched() {
    // Real hianime pools where the bare title and a same-year sequel
    // both confirm Kitsu's year, and Kitsu keeps each as its own
    // entry: whichever entry is asked for, one candidate is it.
    let pools: [(&str, u32, &str, u32, &[u32]); 4] = [
        ("Golden Kamuy", 12, "Golden Kamuy: Season 2", 12, &[12]),
        ("Spy x Family", 12, "Spy x Family, Part 2", 13, &[12, 13]),
        (
            "Mushoku Tensei: Jobless Reincarnation",
            11,
            "Mushoku Tensei: Jobless Reincarnation Part 2",
            12,
            &[11, 12],
        ),
        ("Tokyo Ghoul:re", 12, "Tokyo Ghoul:re 2nd Season", 12, &[12]),
    ];
    for (first, n1, second, n2, expecteds) in pools {
        let cands = [cand(first, n1, true), cand(second, n2, true)];
        for &expected in expecteds {
            assert_eq!(
                split_chain(&cands, expected, best_single(&cands, expected), &[]),
                None,
                "{first} / {second} against {expected}"
            );
        }
    }
}

#[test]
fn a_provider_entry_spanning_two_kitsu_entries_is_not_a_split() {
    // The inverse shape: hianime lists both of Kitsu's Silver Soul
    // halves as one 26-episode show beside the second half alone, and
    // both of Haikyuu's To the Top cours as "(Part 1+2)" beside the
    // second cour. Neither title names a part of the other, so no
    // chain forms whichever entry is asked for.
    let gintama = [
        cand("Gintama.: Silver Soul Arc", 26, true),
        cand("Gintama.: Silver Soul Arc - Second Half War", 14, true),
    ];
    for expected in [12, 14] {
        assert_eq!(
            split_chain(&gintama, expected, best_single(&gintama, expected), &[]),
            None
        );
    }
    let haikyuu = [
        cand("Haikyuu!!: To the Top (Part 1+2)", 25, true),
        cand("Haikyuu!!: To the Top 2nd Season", 12, true),
    ];
    for expected in [13, 12] {
        assert_eq!(
            split_chain(&haikyuu, expected, best_single(&haikyuu, expected), &[]),
            None
        );
    }
}

/// A probed pool: titles drawn from one stem's parts and some
/// strangers, with arbitrary counts, years and offsets.
fn pool() -> impl proptest::strategy::Strategy<Value = Vec<(String, u32, bool, u32)>> {
    let title = proptest::prop_oneof![
        proptest::strategy::Just("The Show".to_string()),
        (2u32..5).prop_map(|k| format!("The Show Part {k}")),
        proptest::strategy::Just("Another Show".to_string()),
        proptest::strategy::Just("The Show Movie".to_string()),
    ];
    proptest::collection::vec(
        (
            title,
            0u32..30,
            proptest::bool::ANY,
            proptest::prop_oneof![proptest::strategy::Just(0u32), 1u32..50],
        ),
        0..7,
    )
}

use proptest::strategy::Strategy as _;

proptest::proptest! {
    #[test]
    fn a_chain_is_distinct_confirmed_parts_led_by_a_plainly_numbered_stem(
        raw in pool(),
        expected in 1u32..40,
    ) {
        let cands: Vec<PartCandidate<'_>> = raw
            .iter()
            .map(|(t, count, confirmed, offset)| PartCandidate {
                title: t,
                count: *count,
                confirmed: *confirmed,
                offset: *offset,
            })
            .collect();
        let best_single = cands
            .iter()
            .map(|c| c.count.abs_diff(expected))
            .min()
            .unwrap_or(u32::MAX);
        let Some(chain) = split_chain(&cands, expected, best_single, &[]) else {
            return Ok(());
        };
        proptest::prop_assert!(chain.len() >= 2);
        let mut seen = chain.clone();
        seen.sort_unstable();
        seen.dedup();
        proptest::prop_assert_eq!(seen.len(), chain.len());
        let lead = &cands[chain[0]];
        proptest::prop_assert_eq!(lead.offset, 0);
        for (k, &i) in chain.iter().enumerate() {
            let c = &cands[i];
            proptest::prop_assert!(c.confirmed && c.count > 0);
            let ordinal = u32::try_from(k + 1).expect("small");
            proptest::prop_assert_eq!(part_ordinal(lead.title, c.title), Some(ordinal));
        }
        // It explains the count strictly better than any single one.
        let sum: u32 = chain.iter().map(|&i| cands[i].count).sum();
        proptest::prop_assert!(sum.abs_diff(expected) < best_single);
        // And a lead that fits alone never heads a chain where some
        // single candidate fits.
        let tolerance = crate::commands::play_native::ep_count_threshold(expected);
        if best_single <= tolerance {
            proptest::prop_assert!(lead.count.abs_diff(expected) > tolerance);
        }
    }

    #[test]
    fn stitched_slots_are_the_positions_whatever_the_tags(
        parts in proptest::collection::vec(
            proptest::collection::vec(
                (1u32..60, proptest::option::of(proptest::prop_oneof![
                    (1u32..60).prop_map(|n| n.to_string()),
                    (1u32..60).prop_map(|n| format!("{n}.5")),
                    "\\PC{0,6}",
                ])),
                0..12,
            ),
            1..4,
        ),
    ) {
        let mut next_id = 0u64;
        let listings: Vec<Vec<EpisodeRef>> = parts
            .iter()
            .map(|rows| {
                rows.iter()
                    .map(|(number, tag)| {
                        next_id += 1;
                        row(next_id, *number, tag.as_deref())
                    })
                    .collect()
            })
            .collect();
        let refs: Vec<&[EpisodeRef]> = listings.iter().map(Vec::as_slice).collect();
        let merged = merge_parts(&refs);
        let slots: Vec<u32> = merged.iter().map(|e| e.number).collect();
        let want: Vec<u32> = (1..=u32::try_from(merged.len()).expect("small")).collect();
        proptest::prop_assert_eq!(slots, want);
        let ids: Vec<u64> = merged.iter().map(|e| e.id).collect();
        let want_ids: Vec<u64> = (1..=next_id).collect();
        proptest::prop_assert_eq!(ids, want_ids);
        // A tag is carried only where it says something the slot does not.
        for e in &merged {
            if let Some(tag) = &e.number2 {
                proptest::prop_assert_ne!(tag, &e.number.to_string());
            }
        }
    }
}
