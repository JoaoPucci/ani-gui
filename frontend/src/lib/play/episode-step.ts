/**
 * Stepping between episodes: by one between whole episodes, and from
 * a fractional extra through the extras listed beside the same
 * episode before the whole episodes either side (see
 * docs/title-resolution.md, "Episode numbers"). Re-exported from
 * `episode-number`, where every reader finds it.
 */

/** The episode before `episode`: one less for a whole episode; for a
 *  fractional extra, the listed extra before it beside the same
 *  episode (12.2 → 12.1), else the whole episode below (7.5 → 7). May
 *  be 0, which is no episode. `extras` are the fractional extras the
 *  provider lists. */
export function previousEpisode(episode: number, extras: readonly number[] = []): number {
	if (Number.isInteger(episode)) return episode - 1;
	const before = siblings(episode, extras).filter((e) => e < episode);
	return before.length > 0 ? Math.max(...before) : Math.floor(episode);
}

/** The episode after `episode`: one more for a whole episode; for a
 *  fractional extra, the listed extra after it beside the same episode
 *  (12.1 → 12.2), else the whole episode above (7.5 → 8). `extras` are
 *  the fractional extras the provider lists. */
export function nextEpisode(episode: number, extras: readonly number[] = []): number {
	if (Number.isInteger(episode)) return episode + 1;
	const after = siblings(episode, extras).filter((e) => e > episode);
	return after.length > 0 ? Math.min(...after) : Math.ceil(episode);
}

/** The listed extras beside the same whole episode as `extra` (12.1
 *  and 12.2 both follow 12). */
function siblings(extra: number, extras: readonly number[]): number[] {
	const anchor = Math.floor(extra);
	return extras.filter((e) => !Number.isInteger(e) && Math.floor(e) === anchor);
}

/** Whether stepping to `target` stays among what can be played: up to
 *  the `cap` (no cap is no bound), or one of the listed `extras`. */
export function stepAllowed(
	target: number,
	cap: number | null,
	extras: readonly number[] = []
): boolean {
	return cap === null || target <= cap || extras.includes(target);
}

/** The provider's extra tags as numbers, dropping any that is not. */
export function extraNumbers(tags: readonly string[]): number[] {
	return tags.map((t) => Number(t)).filter((n) => Number.isFinite(n) && n > 0);
}
