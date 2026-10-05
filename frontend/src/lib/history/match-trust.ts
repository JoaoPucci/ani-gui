/**
 * Which Kitsu ids a play may record on the history row it writes.
 *
 * A Continue card resolves its row to a Kitsu entry, and a play
 * started from that card records the entry's id on the row — after
 * which the row is matched by that id and nothing else. An id is only
 * worth recording when the match behind it is one the user stands
 * behind: the id the row already recorded, or a mapping a real play
 * stored. A title-search guess is not, and recording it would pin the
 * guess to the row with one click.
 */

/** Forget every match noted so far; the home page calls it before it
 *  resolves its rows again. */
export function forgetMatchTrust(): void {}

/** Note how a Continue row's match was reached. */
export function noteMatchTrust(kitsuId: string, trusted: boolean): void {
	void kitsuId;
	void trusted;
}

/** The id a play may record on the row, or undefined when the id is
 *  only a guess the home page made. */
export function recordableKitsuId(kitsuId: string): string | undefined {
	return kitsuId || undefined;
}
