/**
 * What the home page draws for each of its rows, top to bottom:
 * Continue watching, Watch later, Trending now and Top rated. The hero
 * above them keeps one height whatever it shows, so it is not one of
 * them.
 *
 * Each row is loaded on its own and lands when it lands. The page hands
 * this what each one has come to, and draws what it says.
 */

/** What a draw of one row comes to. */
export type HomeRowKind = 'held' | 'skeleton' | 'shown' | 'absent';

/** A list the page fetched: its items once they are in, or that the
 *  fetch failed. */
export interface ListInput {
	items: readonly unknown[] | null;
	failed: boolean;
}

export interface WatchLaterInput {
	/** Whether the saved sign-ins have been read. */
	accountsKnown: boolean;
	/** Whether the viewer is signed in to a tracker the row reads. */
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

function list(input: ListInput): HomeRowKind {
	if (input.items === null && !input.failed) return 'skeleton';
	return input.items && input.items.length > 0 ? 'shown' : 'absent';
}

/** What the home page draws for each row. */
export function homeRows(input: HomeRowsInput): HomeRows {
	const wl = input.watchLater;
	const hasCards = wl.cards !== null && wl.cards.length > 0;
	const failedRow = wl.signedIn && wl.failed && !hasCards;
	const watchLaterShown =
		failedRow || (wl.cards !== null && (hasCards || (wl.signedIn && !wl.failed)));
	return {
		continue: input.history && input.history.length > 0 ? 'shown' : 'absent',
		watchLater: watchLaterShown ? 'shown' : 'absent',
		trending: list(input.trending),
		topRated: list(input.topRated)
	};
}
