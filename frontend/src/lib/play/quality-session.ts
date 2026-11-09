/**
 * How long a level picked in the player's Quality menu lasts: the
 * player session. It carries to the next episode the player moves to,
 * and is let go when the player is left or turns to another show, so
 * the next visit, and every other show, starts on Auto.
 */

import { qualityPin } from './quality-pin';

/** The show the player is showing, or null when it was left. */
let showing: string | null = null;

/** The player's session, as the play page reports it. */
export const playerSession = {
	/** The page shows `showId`'s stream. */
	shows(showId: string): void {
		if (showing !== null && showing !== showId) qualityPin.clear();
		showing = showId;
	},
	/** The page was left. */
	ends(): void {
		qualityPin.clear();
		showing = null;
	}
};
