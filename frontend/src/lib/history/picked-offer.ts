/**
 * The way to change again the show of a line the user named one for —
 * split from $lib/history/link-offer.ts for the per-file complexity bar.
 * A line the history marks as named (`named`, the backend's link note,
 * kept across restarts) is offered on every surface; on the player, a
 * line picked there is offered too until the app restarts
 * ($lib/history/guess-slip.ts).
 */

import type { HistoryEntry, ReadOptions } from '$lib/api';
import type { LinkDeps, LinkOffer } from './link-offer';
import { pickedRow } from './guess-slip';
import { readOpts } from './read-opts';

/** The offer to change again the show of `row`, while it records
 *  `pageId`, the show the surface shows; only for a row `offered`
 *  accepts. A failure to read makes none, and so does a read cancelled
 *  with the visit `opts` names. */
export async function pickedOffer(
	deps: LinkDeps,
	row: string,
	pageId: string,
	offered: (entry: HistoryEntry) => boolean = () => true,
	opts?: ReadOptions
): Promise<LinkOffer | null> {
	try {
		const entry = (await deps.history(...readOpts(opts ?? {}))).find((e) => e.id === row);
		if (!entry || !offered(entry)) return null;
		const { match } = await deps.resolve(entry, ...readOpts(opts ?? {}));
		return match && match.id === pageId ? { entry, current: match, saved: pageId } : null;
	} catch {
		return null;
	}
}

/** The offer to change it again that the surface of the page `pageId`
 *  makes for `row`: for a row the user named a show for, anywhere, and
 *  on the player (`inPlayer`) for one picked there since the app
 *  started; null otherwise. */
export function offerAgain(
	deps: LinkDeps,
	row: string | null,
	pageId: string,
	inPlayer: boolean,
	opts?: ReadOptions
): Promise<LinkOffer | null> {
	if (!row) return Promise.resolve(null);
	const picked = inPlayer && pickedRow(row);
	return pickedOffer(deps, row, pageId, (entry) => picked || entry.named === true, opts);
}
