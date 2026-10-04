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
    }
}

#[test]
fn the_stem_is_the_first_part() {
    assert_eq!(part_ordinal(SBR, SBR), Some(1));
    assert_eq!(part_ordinal(SBR, &SBR.to_uppercase()), Some(1));
}

#[test]
fn a_short_ordinal_suffix_names_a_later_part() {
    assert_eq!(part_ordinal(SBR, &format!("{SBR} 2nd Stage")), Some(2));
    assert_eq!(part_ordinal("The Show", "The Show Part 3"), Some(3));
    assert_eq!(part_ordinal("The Show", "The Show: Season 2"), Some(2));
    assert_eq!(part_ordinal("The Show", "The Show - Second Cour"), Some(2));
    assert_eq!(part_ordinal("The Show", "The Show II"), Some(2));
}

#[test]
fn anything_else_is_not_a_part() {
    assert_eq!(part_ordinal("The Show", "The Show Movie"), None);
    assert_eq!(part_ordinal("The Show", "The Show - The Calamity"), None);
    assert_eq!(part_ordinal("The Show", "Another Show 2nd Season"), None);
    assert_eq!(part_ordinal("The Show", "The Showdown 2"), None);
    // A long tail is a different show that happens to start the same.
    assert_eq!(
        part_ordinal("The Show", "The Show 2nd Season Recap Special Edition"),
        None
    );
}

#[test]
fn an_airing_split_chains_both_parts_in_order() {
    // The provider lists the 2nd Stage first; the chain still starts
    // at the stem.
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(&second, 2, true), cand(SBR, 1, true)];
    assert_eq!(split_chain(&cands, 12, 10), Some(vec![1, 0]));
}

#[test]
fn a_finished_split_beats_the_near_miss_single_part() {
    // 1 + 11 = 12 exactly, where the 2nd Stage alone is one off and
    // would otherwise win inside the tolerance.
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 11, true)];
    assert_eq!(split_chain(&cands, 12, 1), Some(vec![0, 1]));
}

#[test]
fn three_parts_chain_in_order() {
    let cands = [
        cand("The Show Part 3", 4, true),
        cand("The Show", 4, true),
        cand("The Show Part 2", 4, true),
    ];
    assert_eq!(split_chain(&cands, 12, 8), Some(vec![1, 2, 0]));
}

#[test]
fn no_chain_where_a_single_part_already_fits() {
    let cands = [
        cand("The Show", 12, true),
        cand("The Show 2nd Season", 1, true),
    ];
    assert_eq!(split_chain(&cands, 12, 0), None);
    // Still none when the chain only ties the single part.
    let cands = [
        cand("The Show", 11, true),
        cand("The Show 2nd Season", 2, true),
    ];
    assert_eq!(split_chain(&cands, 12, 1), None);
}

#[test]
fn no_chain_without_both_years_confirmed() {
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 2, false)];
    assert_eq!(split_chain(&cands, 12, 10), None);
    let cands = [cand(SBR, 1, false), cand(&second, 2, true)];
    assert_eq!(split_chain(&cands, 12, 10), None);
}

#[test]
fn a_sibling_with_nothing_listed_adds_nothing() {
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 0, true)];
    assert_eq!(split_chain(&cands, 12, 11), None);
}

#[test]
fn no_chain_past_the_expected_count_outside_the_tolerance() {
    let cands = [
        cand("The Show", 12, true),
        cand("The Show Part 2", 12, true),
    ];
    assert_eq!(split_chain(&cands, 12, 0), None);
    let cands = [
        cand("The Show", 10, true),
        cand("The Show Part 2", 10, true),
    ];
    assert_eq!(split_chain(&cands, 12, 2), None);
}

#[test]
fn a_gap_in_the_parts_ends_the_chain() {
    let cands = [cand("The Show", 4, true), cand("The Show Part 3", 4, true)];
    assert_eq!(split_chain(&cands, 12, 8), None);
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
