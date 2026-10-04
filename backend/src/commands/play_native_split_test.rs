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
    assert_eq!(split_chain(&cands, 12, 10, SBR), Some(vec![1, 0]));
}

#[test]
fn a_finished_split_beats_the_near_miss_single_part() {
    // 1 + 11 = 12 exactly, where the 2nd Stage alone is one off and
    // would otherwise win inside the tolerance.
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 11, true)];
    assert_eq!(split_chain(&cands, 12, 1, SBR), Some(vec![0, 1]));
}

#[test]
fn three_parts_chain_in_order() {
    let cands = [
        cand("The Show Part 3", 4, true),
        cand("The Show", 4, true),
        cand("The Show Part 2", 4, true),
    ];
    assert_eq!(split_chain(&cands, 12, 8, "The Show"), Some(vec![1, 2, 0]));
}

#[test]
fn no_chain_where_a_single_part_already_fits() {
    let cands = [
        cand("The Show", 12, true),
        cand("The Show 2nd Season", 1, true),
    ];
    assert_eq!(split_chain(&cands, 12, 0, "The Show"), None);
    // Still none when the chain only ties the single part.
    let cands = [
        cand("The Show", 11, true),
        cand("The Show 2nd Season", 2, true),
    ];
    assert_eq!(split_chain(&cands, 12, 1, "The Show"), None);
}

#[test]
fn no_chain_without_both_years_confirmed() {
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 2, false)];
    assert_eq!(split_chain(&cands, 12, 10, SBR), None);
    let cands = [cand(SBR, 1, false), cand(&second, 2, true)];
    assert_eq!(split_chain(&cands, 12, 10, SBR), None);
}

#[test]
fn a_sibling_with_nothing_listed_adds_nothing() {
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 0, true)];
    assert_eq!(split_chain(&cands, 12, 11, SBR), None);
}

#[test]
fn no_chain_past_the_expected_count_outside_the_tolerance() {
    let cands = [
        cand("The Show", 12, true),
        cand("The Show Part 2", 12, true),
    ];
    assert_eq!(split_chain(&cands, 12, 0, "The Show"), None);
    let cands = [
        cand("The Show", 10, true),
        cand("The Show Part 2", 10, true),
    ];
    assert_eq!(split_chain(&cands, 12, 2, "The Show"), None);
}

#[test]
fn a_gap_in_the_parts_ends_the_chain() {
    let cands = [cand("The Show", 4, true), cand("The Show Part 3", 4, true)];
    assert_eq!(split_chain(&cands, 12, 8, "The Show"), None);
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
    assert_eq!(split_chain(&cands, 13, 1, "X"), None);
}

#[test]
fn a_near_miss_later_part_is_stitched_only_under_the_entrys_own_title() {
    // The finished split is stitched over a later part that sits one
    // off because the entry is named as the bare stem. Searched as the
    // later part's own title, that same pool is the later part's
    // entry, and its near miss stands.
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 11, true)];
    assert_eq!(split_chain(&cands, 12, 1, &second), None);
    assert_eq!(
        split_chain(&cands, 12, 1, &SBR.to_lowercase()),
        Some(vec![0, 1])
    );
}

#[test]
fn an_airing_split_found_through_an_alias_is_still_stitched() {
    // With no single candidate inside the tolerance there is no
    // near miss to protect, and the name searched is only an alias.
    let second = format!("{SBR} 2nd Stage");
    let cands = [cand(SBR, 1, true), cand(&second, 2, true)];
    assert_eq!(
        split_chain(&cands, 12, 10, "JoJo's Bizarre Adventure: Steel Ball Run"),
        Some(vec![0, 1])
    );
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
    assert_eq!(split_chain(&cands, 12, 10, "The Show"), None);
}
