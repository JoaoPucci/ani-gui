/**
 * The lines a guessed Continue card's play went out for in this run of
 * the app, and the show each named.
 *
 * A session a guessed card opened keeps the card's verdict for its
 * life ($lib/history/link-offer.ts, `carried`): its player and the show
 * page it goes up to ask about the line even once the session's own
 * play recorded the guess on it. The verdict travels in the session's
 * URL, and a URL stays in navigation history, so it stands only while
 * this note does: from the guessed play of the line until its show is
 * played any other way — from its own page, or by a player no guessed
 * card opened, there or handed off from it — which may record it on the
 * line as the user's own choice. A play of the show that lands on
 * another line lets go of the note too: the carried verdict then no
 * longer asks about this line's recorded guess, though nothing was
 * written to it (a line still unrecorded is asked about as any guess
 * is). A later guessed play of the line stands for it again, so the
 * next episode of a guessed player revisited asks once more. Every watch
 * names its play here as it is sent, and every handoff once its player
 * started (`$lib/api`), so a slow handoff that started last is the
 * line's latest play. The note lasts until the app restarts, as the
 * URLs it vouches for do.
 */

/** A play's names: its page (`kitsu_id`), and for a guessed card's
 *  play the card's row and its guess. */
export interface PlayNames {
	kitsu_id?: string;
	guess_row?: string;
	guess_id?: string;
}

const lines = new Map<string, string>();

/** Note the play `names` as it goes out. */
export function notePlayed(names: PlayNames): void {
	if (names.guess_row) {
		// A guessed play names its guess apart, or as its page once it
		// landed on the card's own listing.
		const show = names.guess_id ?? names.kitsu_id;
		if (show) lines.set(names.guess_row, show);
		return;
	}
	const played = names.kitsu_id;
	if (!played) return;
	for (const [row, show] of lines) if (show === played) lines.delete(row);
}

/** Whether a guessed card's play of `row` as `showId` went out in this
 *  run, with no other play of that show since. */
export function guessPlayed(row: string, showId: string): boolean {
	return lines.get(row) === showId;
}

/** Test seam: forget every note. */
export function __resetGuessPlaysForTests(): void {
	lines.clear();
}
