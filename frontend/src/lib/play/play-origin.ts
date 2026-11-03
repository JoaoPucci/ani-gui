/**
 * Where a play session came from, carried in its own URL.
 *
 * A play started from a Continue card whose Kitsu match was only a
 * guess — a remembered title match, a search pick, a guessed mapping —
 * records that guess, on the history row and on the user's tracker
 * accounts, only when the play landed on the card's own listing
 * (`guessBinds`); any other play of it would match the row by a guess
 * for good. The flag and the card's row travel with the session's URL (every
 * URL the play page builds for itself — each episode switch — carries
 * it), so the session keeps its verdict for its whole life whatever
 * the home page does meanwhile. A session opened from the
 * detail page, where the user chose the show, never carries it.
 */

const GUESS_PARAM = 'guess';
const ROW_PARAM = 'row';

/** Whether the play URL's query marks a session opened from a guess. */
export function openedFromGuess(search: URLSearchParams): boolean {
	return search.get(GUESS_PARAM) === '1';
}

/** The Kitsu id a request from the session may record, or undefined
 *  for a session opened from a guess. */
export function recordableId(id: string, fromGuess: boolean): string | undefined {
	return fromGuess || !id ? undefined : id;
}

/** Whether a session's play may record its Kitsu id and write the
 *  trackers. A session no guess opened always may. A guessed one may
 *  once its play landed on the card's own listing — the backend's
 *  `own_listing`, answered for the row its plays name (`guessRow`):
 *  the provider listing the guessed show resolves to is the one the
 *  card's history row was written by, so the guess names the show the
 *  row has been playing all along. */
export function guessBinds(guess: boolean, session: { own_listing?: boolean }): boolean {
	return !guess || session.own_listing === true;
}

/** The history row a guessed session's plays name, so the backend can
 *  say whether each landed on that row's own listing; undefined for
 *  every other session. */
export function guessRow(guess: boolean, row: string | null): string | undefined {
	return guess && row ? row : undefined;
}

/** `query` (a `?…` play query) carrying the guess flag when `guess`. */
export function withGuess(query: string, guess: boolean): string {
	if (!guess) return query;
	return `${query}${query.length > 1 ? '&' : ''}${GUESS_PARAM}=1`;
}

/** The history row — its provider show id — whose Continue card
 *  opened the session, or null for a session the detail page opened.
 *  The session's positions are written for it (watch-position.ts), and
 *  every URL the play page builds for itself carries it on. */
export function openedFromRow(search: URLSearchParams): string | null {
	return search.get(ROW_PARAM) || null;
}

/** `query` (a `?…` play query) carrying the history row `row`. */
export function withRow(query: string, row: string | null): string {
	if (!row) return query;
	const param = `${ROW_PARAM}=${encodeURIComponent(row)}`;
	return `${query}${query.length > 1 ? '&' : ''}${param}`;
}
