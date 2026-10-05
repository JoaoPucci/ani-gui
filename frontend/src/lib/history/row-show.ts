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

/** The show `row` is a card of — its resolved match, else its stamped
 *  mapping — or null when it cannot be told. */
export async function rowShow(row: HistoryEntry, deps: RowShowDeps): Promise<string | null> {
	const resolved = deps.matches[row.id]?.id;
	if (resolved) return resolved;
	// No way to ask, a failed lookup and no stamped mapping all leave
	// the row's show untold.
	return (await deps.kitsuIdOf?.(row.id).catch(() => null)) ?? null;
}

/** The Kitsu ids a removed row's positions can be kept under. A
 *  mapping that cannot be read names nothing. */
export async function rowShowIds(row: HistoryEntry, deps: RowShowDeps): Promise<string[]> {
	const show = await rowShow(row, deps);
	return show ? [show] : [];
}
