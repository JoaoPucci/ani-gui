/**
 * The way to change again the show of a line the user named one for —
 * split from $lib/history/link-offer.ts for the per-file complexity bar.
 * A line the history marks as named (`named`, the backend's link note,
 * kept across restarts) is offered on every surface; on the player, a
 * line picked there is offered too until the app restarts
 * ($lib/history/guess-slip.ts).
 */

import type { HistoryEntry, ReadOptions } from '$lib/api';
import { linkOffer, type LinkDeps, type LinkOffer } from './link-offer';
import { pickedRow } from './guess-slip';
import { guessPlayed } from './guess-plays';
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

/** What a surface is, for its offer: whether it may ask about a guess
 *  (`asksGuess`), whether it is the player (`inPlayer`), and whether
 *  its session carries the verdict of the guessed card it was opened
 *  from (`carried`, $lib/history/link-offer.ts `linkOffer`). */
export interface OfferSurface {
	asksGuess: boolean;
	inPlayer: boolean;
	carried?: boolean;
}

/** The offer a surface showing `pageId` makes for the card row `row`:
 *  a guess's when the surface may ask about one (`asksGuess`), else the
 *  way to change again a row the user named a show for (`offerAgain`). */
export async function surfaceOffer(
	deps: LinkDeps,
	row: string | null,
	pageId: string,
	surface: OfferSurface,
	opts?: ReadOptions
): Promise<LinkOffer | null> {
	// A line picked on the player since the app started is named, though
	// its note may not have been read: it is only offered again.
	// The URL's verdict stands only while the line's guessed play does
	// ($lib/history/guess-plays.ts).
	const carried =
		surface.carried === true &&
		row !== null &&
		guessPlayed(row, pageId) &&
		!(surface.inPlayer && pickedRow(row));
	const guess = surface.asksGuess ? await linkOffer(deps, row, pageId, opts, carried) : null;
	return guess ?? offerAgain(deps, row, pageId, surface.inPlayer, opts);
}

/** `surfaceOffer`, and whether it could not be read (`unread`): a
 *  history or match read failed, so no offer is known either way. */
export async function surfaceOfferRead(
	deps: LinkDeps,
	row: string | null,
	pageId: string,
	surface: OfferSurface,
	opts?: ReadOptions
): Promise<{ offer: LinkOffer | null; unread: boolean }> {
	let failed = false;
	const noted = <T>(read: Promise<T>): Promise<T> =>
		read.catch((e: unknown) => {
			failed = true;
			throw e;
		});
	const watched: LinkDeps = {
		...deps,
		history: (...a) => noted(deps.history(...a)),
		resolve: (...a) => noted(deps.resolve(...a))
	};
	const offer = await surfaceOffer(watched, row, pageId, surface, opts);
	return { offer, unread: offer === null && failed };
}
