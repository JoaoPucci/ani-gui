/**
 * The guessed Continue card a show page was opened from, carried in
 * the page's URL. Not wired yet.
 */

/** The query a link to the show page carries for a session a guessed
 *  card opened, or '' for any other session. */
export function guessedQuery(guess: boolean, row: string | null): string {
	void guess;
	void row;
	return '';
}

/** The history row of the guessed card the show page was opened from,
 *  or null. */
export function guessedRowOf(search: URLSearchParams): string | null {
	void search;
	return null;
}
