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

const trusted = new Set<string>();
const guessed = new Set<string>();

/** Forget every match noted so far; the home page calls it before it
 *  resolves its rows again. */
export function forgetMatchTrust(): void {
	trusted.clear();
	guessed.clear();
}

/** Note how a Continue row's match was reached. Two rows can reach one
 *  entry, one by its recorded id and one by a guess; the recorded one
 *  stands. */
export function noteMatchTrust(kitsuId: string, isTrusted: boolean): void {
	(isTrusted ? trusted : guessed).add(kitsuId);
}

/** The id a play may record on the row, or undefined when the id is
 *  only a guess the home page made. An id the home page never resolved
 *  — a play from the detail page, where the user chose the show — is
 *  recorded. */
export function recordableKitsuId(kitsuId: string): string | undefined {
	if (!kitsuId) return undefined;
	return guessed.has(kitsuId) && !trusted.has(kitsuId) ? undefined : kitsuId;
}
