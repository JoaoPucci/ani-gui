import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

/**
 * Drop history rows that resolve to a Kitsu id already claimed by an
 * earlier sibling in the input order. The home page calls this after
 * sortByWatchedAt, so "earlier" means "more recently watched" — the
 * row the user expects when clicking to resume.
 *
 * Why this exists: allmanga catalog drift across ani-cli invocations
 * can produce two `ani-hsts` rows whose alias-walk (see
 * `resolve_allmanga_show_id` in the backend) both land on the same
 * Kitsu entry. Stub-name catalog rows (`1P` for One Piece is the
 * canonical example) and changes in ani-cli's `search_anime`
 * ranking are the usual culprits — the user ends up with two
 * Continue Watching cards for what is logically the same show.
 *
 * Pattern is intentionally defensive: when no two entries share a
 * Kitsu id (the common case), the output is reference-equal row-by-
 * row to the input. The strip renders identically to its pre-dedupe
 * shape.
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
	const seen = new Set<string>();
	const out: HistoryEntry[] = [];
	for (const entry of entries) {
		const match = matches[entry.id];
		if (!match) {
			// Unresolved or null-matched — pass through unchanged.
			out.push(entry);
			continue;
		}
		if (seen.has(match.id)) continue;
		seen.add(match.id);
		out.push(entry);
	}
	return out;
}
