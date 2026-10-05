/**
 * Where a play session came from, carried in its own URL.
 *
 * A play started from a Continue card whose Kitsu match was only a
 * guess — a remembered title match, a search pick, a guessed mapping —
 * must not record that guess anywhere: not on the history row, where
 * the row would be matched by it for good, and not on the user's
 * tracker accounts. The flag travels with the session's URL (every
 * URL the play page builds for itself — each episode switch — carries
 * it), so the session keeps its verdict for its whole life whatever
 * the home page does meanwhile. A session opened from the
 * detail page, where the user chose the show, never carries it.
 */

const GUESS_PARAM = 'guess';

/** Whether the play URL's query marks a session opened from a guess. */
export function openedFromGuess(search: URLSearchParams): boolean {
	return search.get(GUESS_PARAM) === '1';
}

/** The Kitsu id a request from the session may record, or undefined
 *  for a session opened from a guess. */
export function recordableId(id: string, fromGuess: boolean): string | undefined {
	return fromGuess || !id ? undefined : id;
}

/** `query` (a `?…` play query) carrying the guess flag when `guess`. */
export function withGuess(query: string, guess: boolean): string {
	if (!guess) return query;
	return `${query}${query.length > 1 ? '&' : ''}${GUESS_PARAM}=1`;
}
