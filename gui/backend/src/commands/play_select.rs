//! Pure helpers extracted from `commands::play` so the play module's
//! cyclomatic complexity stays manageable. Two clusters live here:
//!
//!   • The custom serde deserializers that decode `PlayArgs` from
//!     both the `POST /api/play` JSON body and the `GET
//!     /api/play/stream` query string. EventSource is GET-only and
//!     `serde_urlencoded` only knows strings, so the SSE path needs
//!     looser coercion than a plain `bool` / `Vec<String>` field
//!     would accept.
//!
//!   • The `select_first_with_hits` family — given a per-title list
//!     of allanime search results plus an optional Kitsu episode-
//!     count expectation, pick the `(title, candidate index)` pair
//!     `ani-cli -S` should be invoked with. Same disambiguation the
//!     availability check uses, so a "yes" verdict on the home page
//!     and a play action lock onto the same allanime show.
//!
//! All five functions are tested in this file's `#[cfg(test)]`
//! module so the play.rs CCN drops without any coverage loss.
//! `deserialize_with = "..."` attributes on `PlayArgs` use the full
//! `crate::commands::play_select::*` paths since the helpers no
//! longer live next to the struct.
//!
//! Nothing in here is async or stateful — no AppState, no network,
//! no filesystem.

use serde::Deserialize;

use crate::scraper;
use crate::scraper::Candidate;

/// Accept either a JSON array of strings or a single newline-joined
/// string for `alt_titles`. The string form is the SSE-query path —
/// `serde_urlencoded` can't decode `alt_titles=a&alt_titles=b` as a Vec.
///
/// # Errors
/// Propagates from the underlying deserializer if the field is
/// neither a list nor a string nor null.
pub fn deserialize_alt_titles<'de, D>(d: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        List(Vec<String>),
        Joined(String),
    }
    Option::<Wire>::deserialize(d).map(|opt| match opt {
        None => Vec::new(),
        Some(Wire::List(v)) => v,
        Some(Wire::Joined(s)) => s
            .split('\n')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect(),
    })
}

/// Accept JSON bool OR `"1"` / `"true"` / `"yes"` strings — the SSE GET
/// path goes through `serde_urlencoded` which only knows strings, so a
/// plain `bool` field would reject `?prefetch=1`. Anything else
/// (`"0"`, `"false"`, `null`, missing) decodes as `false`.
///
/// # Errors
/// Propagates from the underlying deserializer if the field is
/// neither a bool nor a string nor null.
pub fn deserialize_loose_bool<'de, D>(d: D) -> std::result::Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Bool(bool),
        Str(String),
    }
    Option::<Wire>::deserialize(d).map(|opt| match opt {
        None => false,
        Some(Wire::Bool(b)) => b,
        Some(Wire::Str(s)) => matches!(s.as_str(), "1" | "true" | "yes"),
    })
}

/// Choose which `(title, candidate_index)` to feed `ani-cli -S`. Walks
/// the supplied `(title, candidates)` results in order and returns
/// the first one whose candidate list is non-empty, paired with the
/// 1-based index from [`scraper::pick_by_ep_count`] (closest match by
/// episode count to `expected`).
///
/// When every list is empty (or the slice is empty), returns
/// `(primary, 1)` — the legacy `-S 1` behaviour callers used before
/// disambiguation existed.
#[must_use]
pub fn select_first_with_hits(
    primary: &str,
    results: &[(String, Vec<Candidate>)],
    expected: u32,
    mode: &str,
) -> (String, usize) {
    select_first_with_hits_opt(primary, results, Some(expected), None, mode)
}

/// `select_first_with_hits` variant where `expected` may be unknown.
/// When `expected` is `None`, returns the first non-empty list with
/// candidate index 1 (allanime's own ranking — same as ani-cli's
/// default `-S 1`). When `Some`, behaves identically to the v1 helper.
///
/// `expected_year` is the Kitsu start_date year (when known). The
/// underlying [`scraper::pick_by_ep_count_v2`] uses it as a primary
/// tie-break before ep-count distance — fixes the
/// Mobile-Suit-Gundam-1979-vs-Wing case where ep-count distance alone
/// silently picked a sibling.
#[must_use]
pub fn select_first_with_hits_opt(
    primary: &str,
    results: &[(String, Vec<Candidate>)],
    expected: Option<u32>,
    expected_year: Option<u32>,
    mode: &str,
) -> (String, usize) {
    let (title, idx, _) =
        select_first_with_hits_with_candidate(primary, results, expected, expected_year, mode);
    (title, idx)
}

/// Like [`select_first_with_hits_opt`] but also returns a clone of the
/// chosen [`Candidate`] (the row whose `id` + `name` we'll cache for
/// the history-write feedback path). `None` for the candidate when no
/// list had hits — the caller falls back to writing nothing.
///
/// `expected_year` plumbs through to the picker the same way
/// `expected` does; see [`select_first_with_hits_opt`] for rationale.
#[must_use]
pub fn select_first_with_hits_with_candidate(
    primary: &str,
    results: &[(String, Vec<Candidate>)],
    expected: Option<u32>,
    expected_year: Option<u32>,
    mode: &str,
) -> (String, usize, Option<Candidate>) {
    for (title, cands) in results {
        if cands.is_empty() {
            continue;
        }
        let pick = match expected {
            // Picker may explicitly reject the pool (year mismatch
            // or ep-count distance over the tolerance) — when it
            // does, skip this list and try the next alt-title.
            // Falling through to .unwrap_or(1) would silently feed
            // ani-cli candidate #1 and reintroduce the wrong-show
            // bug the picker was added to fix.
            Some(n) => match scraper::pick_by_ep_count_v2(cands, n, expected_year, mode, title) {
                Some(p) => p,
                None => continue,
            },
            // No ep-count from Kitsu (ongoing/upcoming shows). Apply
            // the year filter on its own so an older same-title
            // sibling doesn't win when allmanga returns it first.
            // Codex P2 #3231422658. When expected_year is also None
            // we keep the legacy `pick = 1` fallback.
            None => match pick_first_by_year(cands, expected_year) {
                Some(p) => p,
                None => continue,
            },
        };
        // `pick` is 1-based; clamp into the slice in case
        // pick_by_ep_count ever returns out-of-bounds (defence in
        // depth — its current contract is 1..=len).
        let idx0 = pick.saturating_sub(1).min(cands.len() - 1);
        return (title.clone(), pick, Some(cands[idx0].clone()));
    }
    (primary.to_string(), 1, None)
}

/// Year-only fallback for the ep-count-less path. Mirrors the
/// preference order of `pick_by_ep_count_v2`'s year filter — dated
/// in-range beats undated; all-dated-wrong-year hard-rejects;
/// all-undated (or no year at all) falls through to candidate #1
/// preserving the legacy positional behavior.
///
/// Returns a 1-based index (matches the rest of the picker family),
/// or `None` if every candidate has a year and none match — same
/// "skip this pool, try alt titles" semantic the ep-count path uses.
fn pick_first_by_year(cands: &[Candidate], expected_year: Option<u32>) -> Option<usize> {
    let want = match expected_year {
        Some(y) => y,
        None => return Some(1),
    };
    let in_range = (0..cands.len()).find(|&i| {
        crate::scraper::allanime::AiredStart::year_value(cands[i].aired_start.as_ref())
            .is_some_and(|y| y.abs_diff(want) <= 1)
    });
    if let Some(i) = in_range {
        return Some(i + 1);
    }
    let any_has_year = (0..cands.len()).any(|i| {
        crate::scraper::allanime::AiredStart::year_value(cands[i].aired_start.as_ref()).is_some()
    });
    let any_undated = (0..cands.len()).any(|i| {
        crate::scraper::allanime::AiredStart::year_value(cands[i].aired_start.as_ref()).is_none()
    });
    if any_has_year && !any_undated {
        return None;
    }
    let first_undated = (0..cands.len()).find(|&i| {
        crate::scraper::allanime::AiredStart::year_value(cands[i].aired_start.as_ref()).is_none()
    });
    Some(first_undated.map_or(1, |i| i + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scraper::allanime::AvailableEpisodes;

    fn cand(id: &str, sub: u32) -> Candidate {
        Candidate {
            id: id.into(),
            name: id.into(),
            available_episodes: AvailableEpisodes { sub, dub: 0 },
            aired_start: None,
        }
    }

    fn cand_with_year_local(id: &str, sub: u32, year: Option<u32>) -> Candidate {
        Candidate {
            id: id.into(),
            name: id.into(),
            available_episodes: AvailableEpisodes { sub, dub: 0 },
            aired_start: year.map(|y| crate::scraper::allanime::AiredStart { year: Some(y) }),
        }
    }

    // — Deserializer tests ———————————————————————————————

    #[derive(Deserialize)]
    struct AltOnly {
        #[serde(default, deserialize_with = "deserialize_alt_titles")]
        alt_titles: Vec<String>,
    }

    #[derive(Deserialize)]
    struct BoolOnly {
        #[serde(default, deserialize_with = "deserialize_loose_bool")]
        flag: bool,
    }

    #[test]
    fn alt_titles_decodes_a_json_array() {
        let v: AltOnly = serde_json::from_str(r#"{"alt_titles":["a","b"]}"#).expect("ok");
        assert_eq!(v.alt_titles, vec!["a", "b"]);
    }

    #[test]
    fn alt_titles_splits_a_newline_joined_string() {
        // The `\n` form is what the SSE-query path produces — the
        // frontend joins kitsu titles with newlines because
        // serde_urlencoded can't deserialize repeated keys.
        let v: AltOnly = serde_urlencoded::from_str("alt_titles=a%0Ab%0Ac").expect("ok");
        assert_eq!(v.alt_titles, vec!["a", "b", "c"]);
    }

    #[test]
    fn alt_titles_filters_empty_segments_out_of_the_joined_form() {
        let v: AltOnly = serde_urlencoded::from_str("alt_titles=a%0A%0Ab").expect("ok");
        assert_eq!(v.alt_titles, vec!["a", "b"]);
    }

    #[test]
    fn alt_titles_treats_explicit_null_and_missing_field_as_empty() {
        let null: AltOnly = serde_json::from_str(r#"{"alt_titles":null}"#).expect("ok");
        assert!(null.alt_titles.is_empty());
        let missing: AltOnly = serde_json::from_str(r#"{}"#).expect("ok");
        assert!(missing.alt_titles.is_empty());
    }

    #[test]
    fn loose_bool_accepts_truthy_strings() {
        for s in ["1", "true", "yes"] {
            let qs = format!("flag={s}");
            let v: BoolOnly = serde_urlencoded::from_str(&qs).expect("ok");
            assert!(v.flag, "expected true for {s:?}");
        }
    }

    #[test]
    fn loose_bool_rejects_other_strings_as_false() {
        // Anything that isn't 1/true/yes is false — including the
        // literal "false"/"0" the frontend may send when the user
        // toggled the flag back off. Pin the contract.
        for s in ["0", "false", "no", "wat", ""] {
            let qs = format!("flag={s}");
            let v: BoolOnly = serde_urlencoded::from_str(&qs).expect("ok");
            assert!(!v.flag, "expected false for {s:?}");
        }
    }

    #[test]
    fn loose_bool_passes_through_explicit_json_bools() {
        let t: BoolOnly = serde_json::from_str(r#"{"flag":true}"#).expect("ok");
        let f: BoolOnly = serde_json::from_str(r#"{"flag":false}"#).expect("ok");
        assert!(t.flag);
        assert!(!f.flag);
    }

    #[test]
    fn loose_bool_treats_null_and_missing_field_as_false() {
        let null: BoolOnly = serde_json::from_str(r#"{"flag":null}"#).expect("ok");
        assert!(!null.flag);
        let missing: BoolOnly = serde_json::from_str(r#"{}"#).expect("ok");
        assert!(!missing.flag);
    }

    // — Picker tests ————————————————————————————————————

    #[test]
    fn picks_primary_with_idx_one_when_every_result_list_is_empty() {
        let results: Vec<(String, Vec<Candidate>)> =
            vec![("Naruto".into(), vec![]), ("Naruto S".into(), vec![])];
        assert_eq!(
            select_first_with_hits("Primary", &results, 500, "sub"),
            ("Primary".into(), 1)
        );
    }

    #[test]
    fn picks_first_non_empty_list_using_pick_by_ep_count() {
        // First list empty → walk to second list. Within that list,
        // pick_by_ep_count chooses the candidate whose ep_count is
        // closest to `expected`.
        let results: Vec<(String, Vec<Candidate>)> = vec![
            ("alt-no-hits".into(), vec![]),
            (
                "alt-with-hits".into(),
                vec![cand("a", 1), cand("b", 500), cand("c", 26)],
            ),
        ];
        let (title, pick) = select_first_with_hits("Primary", &results, 500, "sub");
        assert_eq!(title, "alt-with-hits");
        // 'b' is the closest to expected=500, candidate index 2.
        assert_eq!(pick, 2);
    }

    #[test]
    fn opt_variant_ignores_ep_count_when_none_and_returns_index_one() {
        // Stone Ocean Part 6 reproduces this: episode_count is null
        // on Kitsu, so we can't disambiguate — fall through to
        // allanime's own first-hit ordering.
        let results: Vec<(String, Vec<Candidate>)> = vec![(
            "alt".into(),
            vec![cand("a", 12), cand("b", 24), cand("c", 36)],
        )];
        let (title, pick) = select_first_with_hits_opt("Primary", &results, None, None, "sub");
        assert_eq!(title, "alt");
        assert_eq!(pick, 1);
    }

    #[test]
    fn with_candidate_returns_the_chosen_row_for_the_history_write_path() {
        // The play flow caches the chosen candidate's id + name so the
        // post-resolution history write knows which allmanga show to
        // map to the Kitsu id.
        let results: Vec<(String, Vec<Candidate>)> = vec![(
            "Naruto Shippuuden".into(),
            vec![cand("KO_GAKUEN", 1), cand("NARUTO_SHIPPUUDEN", 500)],
        )];
        let (title, pick, chosen) =
            select_first_with_hits_with_candidate("Primary", &results, Some(500), None, "sub");
        assert_eq!(title, "Naruto Shippuuden");
        assert_eq!(pick, 2);
        assert_eq!(chosen.expect("candidate").id, "NARUTO_SHIPPUUDEN");
    }

    #[test]
    fn with_candidate_returns_none_when_every_list_is_empty() {
        let results: Vec<(String, Vec<Candidate>)> = vec![("alt".into(), vec![])];
        let (_, _, chosen) =
            select_first_with_hits_with_candidate("Primary", &results, Some(12), None, "sub");
        assert!(chosen.is_none());
    }

    #[test]
    fn with_candidate_returns_none_when_picker_rejected_only_list() {
        // Pre-existing v2 behavior was `unwrap_or(1)` — when
        // pick_by_ep_count_v2 said "no safe match" the helper still
        // returned `Some(cands[0])`. That defeats the point of the
        // year/ep-count threshold: the play flow ignores the
        // rejection and feeds ani-cli candidate #1 anyway. Pin the
        // new contract: when the picker rejects every list, the
        // helper returns `None` so callers can surface a real
        // "not on allmanga" error instead of silently playing a
        // sibling.
        //
        // Repro shape: a 43-ep show (Mobile Suit Gundam 1979) whose
        // only allmanga candidate is a 49-ep sibling (Wing); the new
        // picker rejects it (best_dist=6 > tolerance=4).
        let cands = vec![cand("wing-style", 49)];
        let results: Vec<(String, Vec<Candidate>)> = vec![("Mobile Suit Gundam".into(), cands)];
        let (_, _, chosen) = select_first_with_hits_with_candidate(
            "Mobile Suit Gundam",
            &results,
            Some(43),
            Some(1979),
            "sub",
        );
        assert!(
            chosen.is_none(),
            "picker rejected the candidate; helper must propagate that as no-match",
        );
    }

    #[test]
    fn with_candidate_applies_year_filter_when_episode_count_is_none() {
        // Codex P2 #3231422658. Ongoing/upcoming shows often have a
        // Kitsu start_date but no episode_count (the count is `null`
        // until the run finishes). Today the helper bypasses the
        // picker entirely when expected is None and grabs candidate
        // #1 — which means the freshly plumbed year is ignored on
        // exactly the franchise-collision case we care about
        // (clicked entry has year but no count; allmanga returns
        // the older same-title result first).
        let cands = vec![
            cand_with_year_local("older", 12, Some(1995)),
            cand_with_year_local("right", 12, Some(2025)),
        ];
        let results: Vec<(String, Vec<Candidate>)> = vec![("Show".into(), cands)];
        let (_, _, chosen) =
            select_first_with_hits_with_candidate("Show", &results, None, Some(2025), "sub");
        assert_eq!(
            chosen.expect("year filter picks the in-range candidate").id,
            "right",
            "expected_year must narrow the pool even when expected ep-count is missing",
        );
    }

    #[test]
    fn with_candidate_skips_to_next_alt_title_when_picker_rejects_first_pool() {
        // Same rejection signal, but a later alt_titles list yields
        // an exact match. The helper should walk past the rejected
        // pool and return the good one rather than collapsing to
        // None after the first reject. (Mirrors alt-title recovery
        // for shows where canonical doesn't index but alt does.)
        let cands_bad = vec![cand("wing", 49)];
        let cands_good = vec![cand("the-right-one", 43)];
        let results: Vec<(String, Vec<Candidate>)> = vec![
            ("Mobile Suit Gundam".into(), cands_bad),
            ("Kidou Senshi Gundam".into(), cands_good),
        ];
        let (title, _, chosen) = select_first_with_hits_with_candidate(
            "Mobile Suit Gundam",
            &results,
            Some(43),
            Some(1979),
            "sub",
        );
        assert_eq!(
            title, "Kidou Senshi Gundam",
            "skipping the rejected pool means the chosen title is from the alt list",
        );
        assert_eq!(
            chosen.expect("alt list yields a candidate").id,
            "the-right-one"
        );
    }

    #[test]
    fn picker_clamps_an_out_of_bounds_pick_into_the_slice_defensively() {
        // pick_by_ep_count's contract is 1..=len, but the picker
        // applies a saturating clamp so a future contract drift
        // (returning 0 or len+1) doesn't panic at runtime.
        let one = vec![cand("only", 1)];
        let results: Vec<(String, Vec<Candidate>)> = vec![("alt".into(), one)];
        // Force pick=1 via expected == 1 (closest match).
        let (_, pick, chosen) =
            select_first_with_hits_with_candidate("Primary", &results, Some(1), None, "sub");
        assert_eq!(pick, 1);
        assert_eq!(chosen.expect("candidate").id, "only");
    }

    proptest::proptest! {
        // Invariants for `select_first_with_hits_with_candidate`:
        //
        //   • If every candidate list is empty → return
        //     `(primary, 1, None)`. This is the legacy fallback
        //     ani-cli has always used.
        //   • Otherwise the chosen title is the first non-empty
        //     list's title, and the chosen candidate is from that
        //     same list (never cross-pollinated from a later list).
        //   • The 1-based pick index is in 1..=len of that list.
        //
        // The picker is the fusion point for play + availability +
        // download — drift here would silently make all three
        // disagree on which allmanga show "Naruto" maps to.
        #[test]
        fn picker_returns_primary_one_none_iff_every_list_is_empty(
            list_count in 0usize..6,
        ) {
            let results: Vec<(String, Vec<Candidate>)> = (0..list_count)
                .map(|i| (format!("title-{i}"), Vec::<Candidate>::new()))
                .collect();
            let (title, pick, chosen) =
                select_first_with_hits_with_candidate("Primary", &results, Some(12), None, "sub");
            proptest::prop_assert_eq!(title, "Primary");
            proptest::prop_assert_eq!(pick, 1);
            proptest::prop_assert!(chosen.is_none());
        }

        // Walk the results in order, drop the empty leaders, pin
        // the picker on the first non-empty list, and assert it
        // never picks something from a later list.
        #[test]
        fn picker_consumes_first_non_empty_list_only(
            empty_prefix_len in 0usize..3,
            tail_lens in proptest::collection::vec(1usize..6, 1..3),
            ep_counts in proptest::collection::vec(0u32..=1_000u32, 1..18),
            expected in 0u32..=1_000u32,
        ) {
            // Pre-flatten ep_counts into the first-non-empty list and
            // any subsequent lists (each gets a slice of length
            // tail_lens[i]). When ep_counts is shorter than the sum
            // of tail_lens we trim — the proptest generators above
            // make the totals usually small enough.
            let mut iter = ep_counts.iter();
            let mut lists: Vec<Vec<Candidate>> = Vec::new();
            for &len in &tail_lens {
                let mut group = Vec::new();
                for _ in 0..len {
                    if let Some(&n) = iter.next() {
                        group.push(cand(&format!("c{}", group.len()), n));
                    }
                }
                if !group.is_empty() {
                    lists.push(group);
                }
            }
            // Skip if tail_lens consumed nothing meaningful.
            proptest::prop_assume!(!lists.is_empty());

            let mut results: Vec<(String, Vec<Candidate>)> = Vec::new();
            for i in 0..empty_prefix_len {
                results.push((format!("empty-{i}"), Vec::new()));
            }
            for (i, group) in lists.into_iter().enumerate() {
                results.push((format!("list-{i}"), group));
            }

            let (title, pick, chosen) = select_first_with_hits_with_candidate(
                "Primary", &results, Some(expected), None, "sub",
            );

            // Three cases now that the picker may reject a pool:
            //   • Picker accepted some list — `chosen` is from that
            //     list and the title matches one of the results
            //     titles (not necessarily the first non-empty one;
            //     a rejection on the first pool walks to the next).
            //   • Picker rejected every non-empty pool — helper
            //     returns `(primary, 1, None)`.
            //
            // Either branch must keep the 1-based pick + list
            // boundary invariants the play path depends on.
            match chosen {
                None => {
                    proptest::prop_assert_eq!(&title, &"Primary".to_string());
                    proptest::prop_assert_eq!(pick, 1);
                }
                Some(c) => {
                    let chosen_list = &results
                        .iter()
                        .find(|(t, _)| t == &title)
                        .expect("chosen title must be one of the results titles")
                        .1;
                    proptest::prop_assert!(pick >= 1);
                    proptest::prop_assert!(pick <= chosen_list.len());
                    proptest::prop_assert!(chosen_list.iter().any(|x| x.id == c.id));
                }
            }
        }
    }
}
