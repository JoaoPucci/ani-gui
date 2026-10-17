/**
 * The engine side of the quality control: a move made on the engine
 * and logged, and a setting that may still be on its way.
 */

import type Hls from 'hls.js';
import type { QualityGovernor, QualityMove } from './quality-governor';
import type { LadderLevel } from './quality-ladder';
import { levelMapping, remapIndex, type RemapWindow } from './quality-remap';
import { repinAfterReplacement } from './quality-steer';
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

/** After an engine error: the engine put back on the level held when
 *  hls.js resumed its own choice — as it does after a level's error,
 *  before any listener runs — and a governor that has nothing left to
 *  hold said so. */
export function afterError(
	engine: Pick<Hls, 'loadLevel' | 'autoLevelEnabled'>,
	governor: QualityGovernor,
	wasEngaged: boolean,
	details: string,
	track: QualityTrack,
	log: (message: string) => void
): void {
	const held = governor.reclaim(engine.autoLevelEnabled);
	if (held !== null) {
		engine.loadLevel = held;
		log(`held at ${track.describe(held)} after ${details}`);
	}
	if (wasEngaged !== governor.isEngaged) {
		log(`released to the engine after ${details}: no level left below the one failing`);
	}
}

/** hls.js removed levels: the track, and the governor once there is
 *  one, carried to the levels left; the removal's window kept open for
 *  the error being dispatched; and, once the dispatch is over, the
 *  engine put back on the level held — a removal outside an error
 *  leaves hls.js on its own choice, or on an index that has shifted. */
export function levelsUpdated(
	engine: Pick<Hls, 'loadLevel' | 'autoLevelEnabled'>,
	governor: QualityGovernor | null,
	track: QualityTrack,
	levels: readonly LadderLevel[],
	removal: RemapWindow,
	log: (message: string) => void
): void {
	const mapping = levelMapping(track.engineLevels, levels);
	removal.opened(
		track.engineLevels,
		levels,
		governor !== null && remapIndex(governor.level, mapping) === null
	);
	track.remap(levels);
	if (!governor) return;
	const wasPinned = governor.isPinned;
	governor.remap(mapping, track.declared());
	repinAfterReplacement(governor, mapping, wasPinned, track.declared());
	log(`levels updated: ${levels.length} left, holding ${track.describe(governor.level)}`);
	queueMicrotask(() => putBack(engine, governor, track, removal, log));
}

function putBack(
	engine: Pick<Hls, 'loadLevel' | 'autoLevelEnabled'>,
	governor: QualityGovernor,
	track: QualityTrack,
	removal: RemapWindow,
	log: (message: string) => void
): void {
	if (removal.isClosed || !governor.isEngaged) return;
	if (!engine.autoLevelEnabled && engine.loadLevel === governor.level) return;
	engine.loadLevel = governor.level;
	log(`held at ${track.describe(governor.level)} after the levels changed`);
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
