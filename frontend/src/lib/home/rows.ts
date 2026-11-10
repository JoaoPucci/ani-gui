/**
 * What the home page draws for each of its rows, top to bottom:
 * Continue watching, Watch later, Trending now and Top rated. The hero
 * above them keeps one height whatever it shows, so it is not one of
 * them.
 *
 * Each row is loaded on its own and lands when it lands. The page hands
 * this what each one has come to, and draws what it says.
 */

import { discoveryView, LOADING, PENDING, type RowLoad } from '$lib/rows/discovery';

/** What a draw of one row comes to: nothing yet, a skeleton, the row,
 *  or nothing at all ($lib/rows/discovery). */
export type HomeRowKind = 'held' | 'skeleton' | 'shown' | 'absent';

/** A list the page fetched: its items once they are in, or that the
 *  fetch failed. */
export interface ListInput {
	items: readonly unknown[] | null;
	failed: boolean;
}

export interface WatchLaterInput {
	/** Whether the saved sign-ins are known: read, or taking long
	 *  enough that the app has said it is waiting for them. */
	accountsKnown: boolean;
	/** Whether the viewer has a sign-in the row reads the list of. */
	signedIn: boolean;
	/** The cards of the last load that landed; null before one has. */
	cards: readonly unknown[] | null;
	/** Whether the last load failed. */
	failed: boolean;
}

export interface HomeRowsInput {
	/** The history rows; null until the read has answered. */
	history: readonly unknown[] | null;
	watchLater: WatchLaterInput;
	/** The trending shows the row lists (the hero's are not among them). */
	trending: ListInput;
	topRated: ListInput;
}

export interface HomeRows {
	continue: HomeRowKind;
	watchLater: HomeRowKind;
	trending: HomeRowKind;
	topRated: HomeRowKind;
}

/** A row the page draws, whatever is in it. */
const THERE: RowLoad<true> = { kind: 'ready', items: [true] };
/** A row the page leaves out. */
const NOT_THERE: RowLoad<true> = { kind: 'ready', items: [] };

function listLoad(input: ListInput): RowLoad<true> {
	if (input.failed) return { kind: 'failed' };
	if (input.items === null) return LOADING;
	return input.items.length > 0 ? THERE : NOT_THERE;
}

/** Continue watching has no skeleton: most viewers with no history
 *  would see one come and go. Until the read answers it is not known
 *  to be there. */
function continueLoad(history: readonly unknown[] | null): RowLoad<true> {
	if (history === null) return PENDING;
	return history.length > 0 ? THERE : NOT_THERE;
}

/** Watch later is there for a viewer with a sign-in it reads, whatever
 *  the list comes to: its cards, its empty state, or its failed state.
 *  Until the sign-ins are known it is not known to be there; once they
 *  are, it is a skeleton while a signed-in list loads. A sign-in left
 *  with cards on screen keeps them until the next load. */
function watchLaterLoad(wl: WatchLaterInput): RowLoad<true> {
	if (!wl.accountsKnown) return PENDING;
	if (wl.cards !== null && wl.cards.length > 0) return THERE;
	if (!wl.signedIn) return NOT_THERE;
	if (wl.cards === null && !wl.failed) return LOADING;
	return THERE;
}

/** What the home page draws for each row. */
export function homeRows(input: HomeRowsInput): HomeRows {
	const [cw, wl, trending, topRated] = discoveryView([
		continueLoad(input.history),
		watchLaterLoad(input.watchLater),
		listLoad(input.trending),
		listLoad(input.topRated)
	]).slots.map((slot) => slot.kind);
	return { continue: cw, watchLater: wl, trending, topRated };
}

/** What the page draws now, given what it drew before: a row already
 *  shown stays shown unless it is dropped. The slot rule takes a row
 *  back to a skeleton when a row above it is loading again, which on
 *  first load cannot happen to a shown row; it can when a row comes in
 *  late above it (Watch later, for a sign-in read after the keyring
 *  notice), and the rows on screen then stay as they are. */
export function keepShown(before: HomeRows | null, next: HomeRows): HomeRows {
	if (before === null) return next;
	const keep = (row: keyof HomeRows): HomeRowKind =>
		before[row] === 'shown' && (next[row] === 'skeleton' || next[row] === 'held')
			? 'shown'
			: next[row];
	return {
		continue: keep('continue'),
		watchLater: keep('watchLater'),
		trending: keep('trending'),
		topRated: keep('topRated')
	};
}
