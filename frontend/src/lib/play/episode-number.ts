/**
 * Episode numbers as the app carries them: a whole episode (7), or a
 * fractional extra a provider lists beside the whole ones (a 7.5
 * recap — see docs/title-resolution.md). Play URLs and history rows
 * carry either as text, and every reader parses it here so a recap
 * stays the recap instead of turning into the episode below it.
 */

/** The episode `raw` names, or `null` when it names none: missing,
 *  blank, not a number, or not positive. */
export function parseEpisodeNumber(raw: string | null | undefined): number | null {
	const text = (raw ?? '').trim();
	if (text === '') return null;
	const n = Number(text);
	return Number.isFinite(n) && n > 0 ? n : null;
}

/** The episode before `episode`: one less for a whole episode, the
 *  whole episode below for a fractional extra (7.5 → 7). May be 0,
 *  which is no episode. `extras` are the fractional extras the
 *  provider lists. */
export function previousEpisode(episode: number, extras: readonly number[] = []): number {
	void extras;
	return Number.isInteger(episode) ? episode - 1 : Math.floor(episode);
}

/** The episode after `episode`: one more for a whole episode, the
 *  whole episode above for a fractional extra (7.5 → 8). `extras` are
 *  the fractional extras the provider lists. */
export function nextEpisode(episode: number, extras: readonly number[] = []): number {
	void extras;
	return Number.isInteger(episode) ? episode + 1 : Math.ceil(episode);
}

/** Whether stepping to `target` stays among what can be played: up to
 *  the `cap` (no cap is no bound), or one of the listed `extras`. */
export function stepAllowed(
	target: number,
	cap: number | null,
	extras: readonly number[] = []
): boolean {
	void extras;
	return cap === null || target <= cap;
}

/** The provider's extra tags as numbers, dropping any that is not. */
export function extraNumbers(tags: readonly string[]): number[] {
	return tags.map((t) => Number(t)).filter((n) => Number.isFinite(n) && n > 0);
}

/** The whole episode a fractional extra follows, for placing it in a
 *  paged strip: 5.5 sits beside 5, and a recap before episode 1 (0.5)
 *  sits beside 1. A whole episode is its own anchor. */
export function stripAnchor(episode: number): number {
	return Math.max(1, Math.floor(episode));
}

/** Whether `episode` belongs on 1-based `page` of a strip paged
 *  `pageSize` at a time — an extra on the page of the episode it
 *  follows, never between two pages. */
export function extraOnPage(episode: number, page: number, pageSize: number): boolean {
	const anchor = stripAnchor(episode);
	return anchor >= (page - 1) * pageSize + 1 && anchor <= page * pageSize;
}

/** The episode a history row names, read by the episode it starts
 *  with: a recap's "7.5" is 7.5, and a range-shaped row ("1-12") is
 *  its first episode, as the history resolver reads it. `null` when
 *  the row names no positive episode. */
export function parseHistoryEpisode(raw: string | null | undefined): number | null {
	const lead = /^\s*(\d+(?:\.\d+)?)/.exec(raw ?? '');
	if (!lead) return null;
	const n = Number(lead[1]);
	return n > 0 ? n : null;
}

/** How many whole episodes are among a page's `tiles` — the count a
 *  page's range and "is this the last page" readings are about; the
 *  fractional extras beside them are not episodes of the run. */
export function wholeEpisodeCount(
	tiles: readonly { number: number | null; relative_number: number | null }[]
): number {
	return tiles.filter((t) => {
		const n = t.number ?? t.relative_number;
		return n !== null && Number.isInteger(n);
	}).length;
}

/** How many columns a strip page's single row needs: one per tile, so
 *  a recap beside a full page does not wrap, and never fewer than a
 *  page, so a short last page keeps the tiles' width. */
export function stripColumns(tileCount: number, pageSize: number): number {
	return Math.max(pageSize, tileCount);
}
