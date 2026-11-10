/**
 * An Undo from Home's notice, and the surface an in-place link session
 * lives on — split from $lib/history/in-place-link.svelte.ts for the
 * per-file complexity bar.
 *
 * The Undo can be pressed, or answer, after the surface moved on: to
 * another card, or away altogether. The rows of its own link are given
 * back wherever the user is, and the undo says what it did. What
 * follows is for the card the link was made on, and only while it is
 * still the one on screen with no other card arrived since: its note
 * goes, and once the undo answered it is offered again. Another card
 * on screen keeps its offer and pick, and the cards are read again. A
 * surface the user left gets nothing.
 */

/** Where an in-place session is: on its surface, or gone from it. */
export interface Surface {
	readonly gone: boolean;
	/** The page left: whatever answers later answers to nothing here. */
	leave: () => void;
	/** The path the session reports as its own: always the same while
	 *  the surface is on screen, somewhere else once it is gone. */
	here: () => string;
}

export function surface(): Surface {
	let gone = false;
	return {
		get gone() {
			return gone;
		},
		leave: () => {
			gone = true;
		},
		here: () => (gone ? 'gone' : '')
	};
}

export interface InPlaceUndo {
	/** The card row the link was made on. */
	row: string;
	surface: Surface;
	/** The card the session is on now, and how many arrivals it had. */
	at: () => { row: string | null; arrival: number };
	/** Drop the card's note of the link, while the offer is decided again. */
	clear: () => void;
	/** Give the rows back, and say what that did or that it failed. */
	undo: () => Promise<void>;
	/** Read again what the surface reads from the history. */
	refresh: () => void;
	/** Decide the card's offer again. */
	offerAgain: () => Promise<unknown>;
}

/** Undo a link made from a card of an in-place surface. */
export async function undoInPlace(u: InPlaceUndo): Promise<void> {
	const { arrival } = u.at();
	const onCard = () => {
		const now = u.at();
		return !u.surface.gone && now.row === u.row && now.arrival === arrival;
	};
	if (onCard()) u.clear();
	await u.undo();
	if (u.surface.gone) return;
	// The history is the truth, whatever the undo answered.
	u.refresh();
	if (onCard()) await u.offerAgain();
}
