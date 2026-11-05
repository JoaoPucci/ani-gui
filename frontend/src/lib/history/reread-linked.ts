/**
 * Home's Continue cards read again in place after a link from one of
 * them, or its undo: the history is read again, and every row whose
 * recorded show changed is matched again — so the linked card names
 * its show, and a row the card no longer stands for comes out from
 * behind it as a card of its own.
 */

import type { HistoryEntry } from '$lib/api';

export interface RereadDeps {
	list: () => Promise<HistoryEntry[]>;
	/** The order Continue Watching shows the rows in. */
	order: (rows: HistoryEntry[]) => HistoryEntry[];
	/** Match the row again and publish what it is now. */
	rematch: (entry: HistoryEntry) => void;
}

/** The history read again in Continue Watching's order, every row
 *  whose recorded show is not what `before` held matched again. */
export async function rereadLinked(
	before: readonly HistoryEntry[],
	deps: RereadDeps
): Promise<HistoryEntry[]> {
	const was = new Map(before.map((h) => [h.id, h.kitsu_id ?? null]));
	const next = deps.order(await deps.list());
	for (const h of next) {
		if (!was.has(h.id) || was.get(h.id) !== (h.kitsu_id ?? null)) deps.rematch(h);
	}
	return next;
}
