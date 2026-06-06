import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

/**
 * Collapse history rows that resolve to the same Kitsu entry down to
 * one per group, keeping the row whose `ep_no` is highest. The home
 * page calls this after `sortByWatchedAt`.
 *
 * Why this exists: allmanga catalog drift across ani-cli invocations
 * can produce two `ani-hsts` rows whose alias-walk (see
 * `resolve_allmanga_show_id` in the backend) both land on the same
 * Kitsu entry. Stub-name catalog rows (`1P` for One Piece is the
 * canonical example) and changes in ani-cli's `search_anime`
 * ranking are the usual culprits — the user ends up with two
 * Continue Watching cards for what is logically the same show.
 *
 * The winner is the row with the most-advanced progress (highest
 * `ep_no`), not the sort-earliest row. The original "first wins"
 * rule hid CLI progress when an older GUI-stamped row sorted above
 * an unstamped CLI row that was actually further along — see Codex
 * P2 #3367725631. Ties on `ep_no` fall back to input order
 * (sortByWatchedAt's most-recent-first), which preserves the
 * intuitive first-wins behaviour for the no-drift case.
 *
 * Pattern stays defensive: when no two entries share a Kitsu id
 * (the common case), the output is reference-equal row-by-row to
 * the input. The strip renders identically to its pre-dedupe shape.
 *
 * Unresolved rows (match === undefined) and null-matched rows are
 * passed through. Two reasons:
 *   1. Per-row release semantics from PR #50: a card stays visible
 *      as a loading placeholder while its match probe is in flight;
 *      we can't know its Kitsu id yet, so we can't dedupe it.
 *   2. Without a Kitsu id (null match: the alias-walk found nothing)
 *      we have no equivalence signal between two such rows. They
 *      might be the same show or might not; rendering both lets the
 *      user decide.
 *
 * Position-preservation: the surviving row emits at the position of
 * its group's first occurrence in the input. So if the input is
 * `[stamped-old, other-show, cli-current]` (sortByWatchedAt put the
 * stamped row at index 0 and the unstamped CLI row at index 2), the
 * output is `[cli-current, other-show]` — the dedupe winner takes
 * the loser's slot and the overall strip ordering is unchanged.
 *
 * One cosmetic side effect when duplicates DO exist: matches arrive
 * out-of-order, so a sort-later sibling might briefly render before
 * its sort-earlier counterpart's match lands. The next derived re-
 * run drops it. Brief (~100ms in practice) and only fires when dupes
 * exist — the price of preserving per-row release.
 */
export function dedupeHistoryByKitsuId(
	entries: HistoryEntry[],
	matches: Record<string, KitsuAnimeRef | null | undefined>
): HistoryEntry[] {
	// Pass 1: pick the winner for each Kitsu group. Highest ep_no
	// wins; ties go to the row encountered first in the input.
	const winnerByKitsuId = new Map<string, HistoryEntry>();
	for (const entry of entries) {
		const match = matches[entry.id];
		if (!match) continue;
		const existing = winnerByKitsuId.get(match.id);
		if (!existing) {
			winnerByKitsuId.set(match.id, entry);
			continue;
		}
		if (parseEpForProgress(entry.ep_no) > parseEpForProgress(existing.ep_no)) {
			winnerByKitsuId.set(match.id, entry);
		}
	}

	// Pass 2: walk in input order, emitting the winner at the slot of
	// its group's first occurrence. Pass-through rows (no match,
	// unresolved match) emit at their original index.
	const out: HistoryEntry[] = [];
	const emittedKitsuIds = new Set<string>();
	for (const entry of entries) {
		const match = matches[entry.id];
		if (!match) {
			out.push(entry);
			continue;
		}
		if (emittedKitsuIds.has(match.id)) continue;
		const winner = winnerByKitsuId.get(match.id);
		out.push(winner ?? entry);
		emittedKitsuIds.add(match.id);
	}
	return out;
}

/** A user-edited or otherwise malformed ep_no (`'abc'`, empty) maps
 *  to a sentinel below any real numeric ep_no so it never beats a
 *  good row in the progress comparison. Real ep counts start at 1;
 *  anything ≤ 0 is also defensively low here. */
function parseEpForProgress(s: string): number {
	const n = parseInt(s, 10);
	return Number.isFinite(n) ? n : -1;
}
