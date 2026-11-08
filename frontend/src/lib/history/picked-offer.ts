/**
 * The player's way to change again the show of a card the viewer
 * picked or confirmed there — split from $lib/history/link-offer.ts
 * for the per-file complexity bar. The row is remembered until the app
 * restarts ($lib/history/guess-slip.ts).
 */

import type { LinkDeps, LinkOffer } from './link-offer';
import { pickedRow } from './guess-slip';

/** The offer the player of a row the viewer picked a show for makes,
 *  to change it again: while the row records `pageId`, the show
 *  playing. A failure to read makes none. */
export async function pickedOffer(
	deps: LinkDeps,
	row: string,
	pageId: string
): Promise<LinkOffer | null> {
	try {
		const entry = (await deps.history()).find((e) => e.id === row);
		if (!entry) return null;
		const { match } = await deps.resolve(entry);
		return match && match.id === pageId ? { entry, current: match, saved: pageId } : null;
	} catch {
		return null;
	}
}

/** The offer to change it again that the player (`inPlayer`) of the
 *  page `pageId` makes for `row`, once the viewer picked a show for it
 *  there; null anywhere else. */
export function offerAgain(
	deps: LinkDeps,
	row: string | null,
	pageId: string,
	inPlayer: boolean
): Promise<LinkOffer | null> {
	return inPlayer && row && pickedRow(row) ? pickedOffer(deps, row, pageId) : Promise.resolve(null);
}
