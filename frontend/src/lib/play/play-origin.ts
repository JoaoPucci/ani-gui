/**
 * Where a play session came from, carried in its own URL.
 *
 * A play started from a Continue card whose Kitsu match was only a
 * guess — a remembered title match, a search pick, a guessed mapping —
 * records that guess, on the history row and on the user's tracker
 * accounts, only when the play landed on the card's own listing
 * (`guessBinds`; for the row a play's resolve writes, the backend's own
 * listing check on `guess_id`, see `guessedId`); any other play of it
 * would match the row by a guess for good. The flag and the card's row travel with the session's URL (every
 * URL the play page builds for itself — each episode switch — carries
 * it), so the session keeps its verdict for its whole life whatever
 * the home page does meanwhile. A session opened from the
 * detail page, where the user chose the show, never carries it.
 */

const GUESS_PARAM = 'guess';
const ROW_PARAM = 'row';
const LANDED_PARAM = 'landed';

/** Whether the play URL's query marks a session opened from a guess. */
export function openedFromGuess(search: URLSearchParams): boolean {
	return search.get(GUESS_PARAM) === '1';
}

/** The page's Kitsu id a request from the session names (`kitsu_id`),
 *  or undefined for a session opened from a guess, whose guess travels
 *  apart (`guessedId`). */
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

/** The Kitsu id a guessed session's plays name as its guess, apart
 *  from the page's (`recordableId` withholds that): the backend records
 *  it on the row a resolve writes only when the play landed on the
 *  card's own listing, so a session whose mark-watched never lands
 *  still names its show. Undefined for every other session. */
export function guessedId(guess: boolean, id: string): string | undefined {
	return guess && id ? id : undefined;
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

/** `query` (a `?…` play query) saying the guessed session's play landed
 *  on another listing than the card's own (`own_listing` false). The
 *  player then offers no "That's right": the guess was never tested
 *  against the card's listing, and keeping it would record the other
 *  listing's show on the card's line. Nothing for any other session. */
export function withLanding(
	query: string,
	guess: boolean,
	session: { own_listing?: boolean }
): string {
	if (!guess || session.own_listing === true) return query;
	return `${query}${query.length > 1 ? '&' : ''}${LANDED_PARAM}=other`;
}

/** Whether the play URL's query says the guessed session's play landed
 *  on another listing than the card's own. */
export function landedElsewhere(search: URLSearchParams): boolean {
	return search.get(LANDED_PARAM) === 'other';
}

/** What a session's plays name as their show, and what its watches are
 *  recorded and mirrored as.
 *
 *  A session no guess opened plays its page's show. A guessed one names
 *  its guess apart, with the card's row. Once the card was linked from
 *  the session (`linked`), its later plays name the link in place of
 *  the guess: the backend records it only on the card's own listing,
 *  never the old guess, and the trackers hear nothing of a stream that
 *  is still the guessed show's. */
export function sessionShow(o: {
	id: string;
	guess: boolean;
	row: string | null;
	linked: string | null;
}): { kitsuId?: string; guessRow?: string; guessId?: string; recordAs: string; mirrors: boolean } {
	if (!o.guess) {
		const kitsuId = recordableId(o.id, false);
		return { ...(kitsuId ? { kitsuId } : {}), recordAs: o.id, mirrors: true };
	}
	const show = o.linked ?? o.id;
	const row = guessRow(true, o.row);
	const id = guessedId(true, show);
	return {
		...(row ? { guessRow: row } : {}),
		...(id ? { guessId: id } : {}),
		recordAs: show,
		mirrors: o.linked === null
	};
}
