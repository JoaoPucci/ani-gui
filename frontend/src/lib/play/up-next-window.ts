/**
 * When the player's "Up next" card shows over a finale: from the start
 * of its ending credits, the point the Skip Ending pill already knows
 * from aniskip, and without that from the last minutes the app counts
 * an episode as finished by (`isFinishedAt`). It stays through the
 * credits and after the video ends; a seek back before that point
 * puts it away again.
 */
import type { SkipInterval } from '$lib/api';

/** Where the ending credits start, from the latest ending interval, or null. */
export function creditsStart(skips: readonly SkipInterval[]): number | null {
	void skips;
	return null;
}

/** Whether the playhead is in a finale's credits, or past them. */
export function inCredits(
	currentTime: number,
	duration: number,
	skips: readonly SkipInterval[]
): boolean {
	void [currentTime, duration, skips];
	return false;
}
