/**
 * Stub: the signatures the tests are written against, with the
 * whole-number behaviour the callers have today.
 */

export function parseEpisodeNumber(raw: string | null | undefined): number | null {
	const n = parseInt(raw ?? '', 10);
	return Number.isFinite(n) && n > 0 ? n : null;
}

export function previousEpisode(episode: number): number {
	return episode - 1;
}

export function nextEpisode(episode: number): number {
	return episode + 1;
}
