//! Resume-by-id resolution for the play flow.
//!
//! Continue Watching hands the play command the exact allanime `show_id`
//! recorded in the history row. This module turns that id back into the
//! `(search_title, 1-based index, candidate)` triple ani-cli needs —
//! selecting the show by identity instead of the title + ep-count
//! heuristic, which can't tell same-title franchise cours apart (Stone
//! Ocean Part 1 vs Part 2: same title, same 12-ep count, and the Kitsu
//! title even drops the disambiguating "Part 6").
//!
//! Lives outside `play.rs` because its only path that matters is a live
//! `fetch_show` + `search` round-trip, which can't be unit-covered
//! offline; keeping it here stops those lines from dragging `play.rs`'s
//! coverage.

use crate::app::AppState;
use crate::commands::play_select::{index_of_show_id, select_by_show_id};
use crate::scraper;
use crate::scraper::Candidate;

/// Locate the exact recorded show by allanime id. Checks the pools the
/// picker already searched first (free), then — only when the id is
/// absent from them — pays for one `fetch_show` plus searches of the
/// show's own names. The Kitsu title the resume sends often drops the
/// franchise marker ("Part 6"), so the id is missing from the title/alt
/// pools; the show's own `name` puts it back. Appends every fresh pool
/// to `results` so the caller's logging/fallthrough sees them. Returns
/// `None` when the id can't be found anywhere — the caller then keeps
/// its heuristic pick.
pub(crate) async fn resolve_by_show_id(
    state: &AppState,
    mode: &str,
    show_id: &str,
    results: &mut Vec<(String, Vec<Candidate>)>,
) -> Option<(String, usize, Candidate)> {
    if let Some(hit) = select_by_show_id(results, show_id) {
        return Some(hit);
    }
    let meta = scraper::allanime::fetch_show(&state.proxy_http, show_id, None)
        .await
        .ok()?;
    let names = std::iter::once(meta.name.clone()).chain(meta.search_terms());
    for name in names {
        if name.trim().is_empty() {
            continue;
        }
        let Ok(cands) = scraper::search(&state.proxy_http, &name, mode, None).await else {
            continue;
        };
        let hit = index_of_show_id(&cands, show_id)
            .map(|idx| (name.clone(), idx, cands[idx - 1].clone()));
        results.push((name, cands));
        if hit.is_some() {
            return hit;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str) -> Candidate {
        Candidate {
            id: id.into(),
            name: id.into(),
            ..Default::default()
        }
    }

    #[test]
    fn no_requested_show_id_keeps_the_heuristic_pick() {
        // Title-based play (search / detail click): the heuristic stands.
        let (t, i, c) = pick_for_requested_show(None, Some(cand("h")), None, "T".into(), 2);
        assert_eq!(
            (t, i, c.map(|c| c.id)),
            ("T".to_string(), 2, Some("h".to_string()))
        );
    }

    #[test]
    fn heuristic_that_already_matches_the_requested_id_is_kept() {
        let (_, _, c) = pick_for_requested_show(Some("x"), Some(cand("x")), None, "T".into(), 1);
        assert_eq!(c.map(|c| c.id), Some("x".to_string()));
    }

    #[test]
    fn exact_match_overrides_a_mismatched_heuristic() {
        let exact = Some(("Real".to_string(), 3, cand("x")));
        let (t, i, c) =
            pick_for_requested_show(Some("x"), Some(cand("wrong")), exact, "T".into(), 1);
        assert_eq!(
            (t, i, c.map(|c| c.id)),
            ("Real".to_string(), 3, Some("x".to_string()))
        );
    }

    #[test]
    fn requested_but_unresolved_drops_the_candidate_instead_of_launching_a_sibling() {
        // Codex P2: a requested show_id that can't be confirmed must NOT
        // fall back to a different show the heuristic happened to pick —
        // the caller surfaces a miss (Network/NoResults) instead.
        let (_, _, c) =
            pick_for_requested_show(Some("x"), Some(cand("wrong")), None, "T".into(), 1);
        assert!(
            c.is_none(),
            "mismatched heuristic must be dropped, not launched"
        );
    }
}
