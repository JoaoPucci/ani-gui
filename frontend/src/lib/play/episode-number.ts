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
 *  which is no episode. */
export function previousEpisode(episode: number): number {
	return Number.isInteger(episode) ? episode - 1 : Math.floor(episode);
}

/** The episode after `episode`: one more for a whole episode, the
 *  whole episode above for a fractional extra (7.5 → 8). */
export function nextEpisode(episode: number): number {
	return Number.isInteger(episode) ? episode + 1 : Math.ceil(episode);
}
