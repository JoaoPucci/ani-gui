/**
 * The guessed Continue card a show page was opened from, carried in
 * the page's URL.
 *
 * A Continue card whose history row recorded no Kitsu id shows the
 * show a guess matched it to, and opens its play as a guess
 * ($lib/play/play-origin.ts). The player's links up to the show's page
 * carry the card's row on, so that page — and only that page — can
 * offer to say which show the row really is ($lib/history/link-offer.ts).
 * A show page opened any other way carries nothing.
 */

const GUESSED_PARAM = 'guessed';

/** The query (`?guessed=<row>`) a link to the show page carries for a
 *  session a guessed card opened, or '' for any other session. */
export function guessedQuery(guess: boolean, row: string | null): string {
	if (!guess || !row) return '';
	return `?${GUESSED_PARAM}=${encodeURIComponent(row)}`;
}

/** The history row of the guessed card the show page was opened from,
 *  or null. */
export function guessedRowOf(search: URLSearchParams): string | null {
	return search.get(GUESSED_PARAM) || null;
}
