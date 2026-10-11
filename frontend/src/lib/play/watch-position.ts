/**
 * Where each episode was left, so opening it again resumes there.
 * Kept in the renderer's local storage under one key, for the most
 * recent episodes only. An episode left in its first seconds is kept
 * at zero — started, to start over — so Continue still goes back to
 * it; one left in its last minutes is finished and forgotten.
 */

import { picks, writes } from './write-order';
import { defaultStorage, keyOf, load, store, type PositionStorage } from './watch-position-storage';

export { MAX_POSITIONS, type PositionStorage } from './watch-position-storage';
export {
	clearAllPositions,
	clearPosition,
	clearRowPositions,
	clearShowPositions,
	snapshotPositions
} from './watch-position-clear';

/** Below this, an episode is kept as started, at zero. */
export const RESUME_MIN_S = 15;
/** With this little left, an episode counts as finished. */
export const FINISHED_REMAINING_S = 90;

/** Records `seconds` into `episode` of `showId` — zero, a started
 *  mark, when that is its start — or forgets the episode when
 *  `duration` puts it at its end. `row` is the history row whose
 *  Continue card opened the session, when one did. */
export function savePosition(
	showId: string,
	episode: number,
	seconds: number,
	duration: number,
	storage: PositionStorage | null = defaultStorage(),
	row: string | null = null
): void {
	const key = keyOf(showId, episode);
	const rest = load(storage).filter(([k]) => k !== key);
	// A point in the first seconds is a start over, never a finish —
	// on a stream shorter than its last 90 seconds as well.
	const point = seconds < RESUME_MIN_S ? 0 : seconds;
	if (point > 0 && isFinishedAt(point, duration)) {
		store(storage, rest);
		return;
	}
	if (store(storage, [...rest, row ? [key, point, row] : [key, point]])) writes.note(storage, key);
}

/** Marks `episode` of `showId` started, at zero, for `row` as
 *  savePosition does, unless a point is already kept for it. A pick
 *  of such a point is noted: a removal's cleanup begun before it,
 *  which forgets the point, leaves a started mark in its place. */
export function markStarted(
	showId: string,
	episode: number,
	storage: PositionStorage | null = defaultStorage(),
	row: string | null = null
): void {
	if (readPosition(showId, episode, storage) === null) {
		savePosition(showId, episode, 0, Number.NaN, storage, row);
	} else {
		picks.note(storage, keyOf(showId, episode));
	}
}

/** Whether `seconds` is in an episode's last minutes, by `duration`.
 *  A length not known yet decides nothing: the point is not finished
 *  as far as anyone can tell, and the first visit that learns the
 *  length asks again. */
export function isFinishedAt(seconds: number, duration: number): boolean {
	return Number.isFinite(duration) && duration - seconds <= FINISHED_REMAINING_S;
}

/** Where `episode` of `showId` was left, or null. */
export function readPosition(
	showId: string,
	episode: number,
	storage: PositionStorage | null = defaultStorage()
): number | null {
	const key = keyOf(showId, episode);
	return load(storage).find(([k]) => k === key)?.[1] ?? null;
}
