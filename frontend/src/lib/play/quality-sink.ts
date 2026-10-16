/**
 * The engine side of the quality control: a move made on the engine
 * and logged, and a setting that may still be on its way.
 */

import type Hls from 'hls.js';
import type { QualityMove } from './quality-governor';
import type { QualityTrack } from './quality-track';

/** Makes a move on the engine and logs it: `next` replaces the buffer
 *  ahead, `load` takes effect from the next segment. */
export function engineMover(
	engine: Pick<Hls, 'loadLevel' | 'nextLevel'>,
	track: QualityTrack,
	log: (message: string) => void
): (move: QualityMove | null) => void {
	return (move) => {
		if (!move) return;
		if (move.via === 'next') engine.nextLevel = move.level;
		else engine.loadLevel = move.level;
		log(track.describeMove(move));
	};
}

/** Hands `then` the value, at once when it is in hand and once it
 *  settles when it is a promise; a promise that fails hands over
 *  `undefined`. */
export function whenSettled<T>(value: T | Promise<T>, then: (value: T | undefined) => void): void {
	if (value instanceof Promise) void value.then(then, () => then(undefined));
	else then(value);
}

/** How long a stream's start waits for the settings. */
export const QUALITY_SETTING_WAIT_MS = 3_000;

/** The value, or undefined when it fails or does not come within
 *  `ms`: settings that never answer cost the start level, not the
 *  start. */
export function settledWithin<T>(value: Promise<T>, ms: number): Promise<T | undefined> {
	const bound = new Promise<undefined>((resolve) => setTimeout(() => resolve(undefined), ms));
	return Promise.race([value.catch(() => undefined), bound]);
}
