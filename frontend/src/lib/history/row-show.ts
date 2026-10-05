/**
 * Which show a Continue row is a card of, for the positions a removed
 * card forgets (watch-position.ts): they are keyed by the Kitsu id the
 * play page had, and a show still on another card keeps them.
 */

import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

export interface RowShowDeps {
	/** Each row's resolved Kitsu match, as the home page holds it. */
	matches: Record<string, KitsuAnimeRef | null | undefined>;
	/** The Kitsu id a play stamped for a row's show id — for a row
	 *  whose match never resolved. */
	kitsuIdOf?: (showId: string) => Promise<string | null>;
	/** Whether Kitsu answers that `kitsuId` is gone (a 404 or 410).
	 *  Rejects when the answer says nothing about the id. */
	recordedGone?: (kitsuId: string) => Promise<boolean>;
}

/** The show `row` is a card of, or null when it cannot be told. The
 *  id the row records names it unless Kitsu answers that id gone —
 *  the rule the Continue resolver and the backend's claim apply. Past
 *  that, the row is the entry the home page resolved for it (its
 *  mapping, else its title match), else its stamped mapping. */
export async function rowShow(row: HistoryEntry, deps: RowShowDeps): Promise<string | null> {
	const resolved = deps.matches[row.id]?.id;
	// The resolver goes past a recorded id only when Kitsu answers it
	// gone, so a resolved match is the row's show either way.
	if (resolved) return resolved;
	if (row.kitsu_id) {
		const gone = await recordedIsGone(row.kitsu_id, deps);
		if (gone === null) return null;
		if (!gone) return row.kitsu_id;
	}
	// No way to ask, a failed lookup and no stamped mapping all leave
	// the row's show untold.
	return (await deps.kitsuIdOf?.(row.id).catch(() => null)) ?? null;
}

/** Whether Kitsu answers `kitsuId` gone; null when the answer says
 *  nothing. With no way to ask, the id stands. */
async function recordedIsGone(kitsuId: string, deps: RowShowDeps): Promise<boolean | null> {
	if (!deps.recordedGone) return false;
	return deps.recordedGone(kitsuId).catch(() => null);
}

/** The Kitsu ids a removed row's positions can be kept under: the id
 *  it records, gone or not — a play keyed them by it while it stood —
 *  and the entry its card was shown as, else its stamped mapping. A
 *  mapping that cannot be read names nothing. */
export async function rowShowIds(row: HistoryEntry, deps: RowShowDeps): Promise<string[]> {
	const ids = new Set<string>();
	if (row.kitsu_id) ids.add(row.kitsu_id);
	const shown =
		deps.matches[row.id]?.id ?? (await deps.kitsuIdOf?.(row.id).catch(() => null)) ?? null;
	if (shown) ids.add(shown);
	return [...ids];
}
