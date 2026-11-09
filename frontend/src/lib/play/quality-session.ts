/**
 * How long a level picked in the player's Quality menu lasts: the
 * player session. It carries to the next episode the player moves to,
 * and is let go when the player is left or turns to another show.
 */

/** The player's session, as the play page reports it. */
export const playerSession = {
	/** The page shows `showId`'s stream. */
	shows(showId: string): void {
		void showId;
	},
	/** The page was left. */
	ends(): void {}
};
