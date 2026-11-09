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

/**
 * Whether the playhead is in a finale's credits, or past them. `skips`
 * is null while the skip times have not answered: the last-minutes
 * rule waits for them, so a late answer cannot take the card back.
 */
export function inCredits(
	currentTime: number,
	duration: number,
	skips: readonly SkipInterval[] | null
): boolean {
	if (skips === null) return false;
	const start = creditsStart(skips);
	if (start !== null) return currentTime >= start;
	// The play page holds a length it has not learned as zero.
	return duration > 0 && isFinishedAt(currentTime, duration);
}

/** How far short of its length a stream may stop and still count as
 *  ended. */
export const END_SLACK_S = 1;

/** Whether the video has ended: the element's own ended state. */
export function atVideoEnd(_currentTime: number, _duration: number, ended: boolean): boolean {
	return ended;
}

/** What the play page knows when it decides whether the card shows. */
export interface UpNextState {
	/** `finaleCard` found something to name. */
	readonly card: boolean;
	/** The player's own next-episode step is open: not the entry's end. */
	readonly hasNext: boolean;
	readonly resumeHolding: boolean;
	readonly dismissed: boolean;
	/** The <video> itself is fullscreen: nothing else on the page is drawn. */
	readonly videoFullscreen: boolean;
	/** The point the card waits for has been reached. */
	readonly atEnd: boolean;
}

/** Whether the Up next card shows. */
export function upNextShown(state: UpNextState): boolean {
	return (
		state.card &&
		!state.hasNext &&
		!state.resumeHolding &&
		!state.dismissed &&
		!state.videoFullscreen &&
		state.atEnd
	);
}
