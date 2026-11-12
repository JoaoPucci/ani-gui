/**
 * The offer a surface makes for a Continue card's line — a guess's, or
 * the way to change again the show of a line the user named one for —
 * and, on the player, the pick the line keeps.
 *
 * The surface reads the line and Continue Watching's match for it, and
 * the backend says what it offers (`commands/link_verdict.rs`). What
 * the surface brings is its own session: whether it may ask about a
 * guess, whether it is the player, whether its session carries the
 * verdict of the guessed card it was opened from while that guessed
 * play stands ($lib/history/guess-plays.ts), and whether the line was
 * picked on the player since the app started ($lib/history/guess-slip.ts).
 * A line the history marks as named (`named`, the backend's link note,
 * kept across restarts) is offered again on every surface.
 */

import type { HistoryEntry, KitsuAnimeRef, ReadOptions } from '$lib/api';
import type { LinkDeps, LinkOffer } from './link-offer';
import { pickedRow } from './guess-slip';
import { guessPlayed } from './guess-plays';
import { readOpts } from './read-opts';

/** What a surface is, for its offer: whether it may ask about a guess
 *  (`asksGuess`), whether it is the player (`inPlayer`), and whether
 *  its session was opened from the card as a guess (`carried`), which
 *  stands only while the line's guessed play does
 *  ($lib/history/guess-plays.ts). */
export interface OfferSurface {
	asksGuess: boolean;
	inPlayer: boolean;
	carried?: boolean;
}

/** What a surface found for a line: its offer, whether the line could
 *  not be read (`unread`: no offer is known either way), and the show
 *  the line was named for when it is not the one shown (`kept`). */
export interface SurfaceFind {
	offer: LinkOffer | null;
	unread: boolean;
	kept: KitsuAnimeRef | null;
}

/** The offer to change again a line that records `match`, made on the
 *  surface showing `pageId`: on that show, which is the match itself or,
 *  for a kept pick on the player, the show playing. One whose show
 *  cannot be read is not made. */
async function againOffer(
	deps: LinkDeps,
	entry: HistoryEntry,
	match: KitsuAnimeRef,
	pageId: string,
	opts?: ReadOptions
): Promise<LinkOffer | null> {
	if (match.id === pageId) return { entry, current: match, saved: pageId };
	const current = await deps.show?.(pageId, ...readOpts(opts ?? {})).catch(() => null);
	return current ? { entry, current, saved: match.id } : null;
}

const NONE: SurfaceFind = { offer: null, unread: false, kept: null };
const UNREAD: SurfaceFind = { offer: null, unread: true, kept: null };

/** What the surface showing `pageId` finds for the card row `row`. A
 *  line gone from the history, or matching no show, finds nothing. A
 *  failure to read, or a read cancelled with the visit `opts` names,
 *  finds no offer, and says it could not read. */
export async function surfaceOfferRead(
	deps: LinkDeps,
	row: string | null,
	pageId: string,
	surface: OfferSurface,
	opts?: ReadOptions
): Promise<SurfaceFind> {
	if (!row) return NONE;
	try {
		const entry = (await deps.history(...readOpts(opts ?? {}))).find((e) => e.id === row);
		if (!entry) return NONE;
		const { match, trusted } = await deps.resolve(entry, ...readOpts(opts ?? {}));
		if (!match) return NONE;
		const verdict = await deps.verdict(
			{
				page: pageId,
				show: match.id,
				trusted,
				named: entry.named === true,
				asks_guess: surface.asksGuess,
				carried: surface.carried === true && guessPlayed(row, pageId),
				picked: surface.inPlayer && pickedRow(row),
				in_player: surface.inPlayer
			},
			...readOpts(opts ?? {})
		);
		const offer =
			verdict.offer === 'guess'
				? { entry, current: match }
				: verdict.offer === 'again'
					? await againOffer(deps, entry, match, pageId, opts)
					: null;
		return { offer, unread: false, kept: verdict.kept ? match : null };
	} catch {
		return UNREAD;
	}
}
