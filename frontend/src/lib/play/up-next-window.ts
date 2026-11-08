/**
 * When the player's "Up next" card shows over a finale: from the start
 * of its ending credits, the point the Skip Ending pill already knows
 * from aniskip, and without that from the last minutes the app counts
 * an episode as finished by (`isFinishedAt`). It stays through the
 * credits and after the video ends; a seek back before that point
 * puts it away again.
 */
import type { SkipInterval } from '$lib/api';
import { isFinishedAt } from './watch-position';

/** Where the ending credits start, from the latest ending interval, or null. */
export function creditsStart(skips: readonly SkipInterval[]): number | null {
	let start: number | null = null;
	for (const s of skips) {
		if (s.skip_type !== 'ed' && s.skip_type !== 'mixed-ed') continue;
		if (start === null || s.start_time > start) start = s.start_time;
	}
	return start;
}

/** Whether the playhead is in a finale's credits, or past them. */
export function inCredits(
	currentTime: number,
	duration: number,
	skips: readonly SkipInterval[]
): boolean {
	const start = creditsStart(skips);
	return start !== null ? currentTime >= start : isFinishedAt(currentTime, duration);
}
