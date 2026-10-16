/**
 * Which level the player loads, decided here rather than by the
 * engine's bandwidth estimate.
 *
 * hls.js left to itself starts from a low estimate of the bandwidth
 * and measures each segment's load to correct it. Every segment
 * reaches it through the app's local proxy, which spaces requests to
 * the host, so the time a segment waits in that line reads as a slow
 * network and the estimate stays low: a `best` setting played the
 * master's lowest level. The governor takes the choice instead. It
 * starts at the level the setting names and stays there while
 * playback is healthy. Playback that stalls with nothing buffered
 * steps it one level down, at most once per cooldown; a level whose
 * playlist cannot be loaded is left for the rest of the stream; and
 * once playback has run healthy for a while with a buffer in hand it
 * steps back up, one level per window, towards the setting's level —
 * the window doubling with every step down, so a level that cannot
 * be held is retried less and less often rather than in a loop.
 *
 * A level the viewer picks is pinned: stalls do not move it, and only
 * a level the engine cannot load moves it down. "Auto" hands the
 * choice back.
 */

import type { LadderLevel, QualityTarget } from './quality-ladder';

/** A stall this soon after a seek is the seek's own wait for media,
 *  not the level failing to keep up. */
export const SEEK_GRACE_MS = 3_000;

/** The least time between two steps down: a step's first segment at
 *  the lower level needs time to arrive, and the stall that caused
 *  the step is not a second reason to take another. */
export const STEP_DOWN_COOLDOWN_MS = 8_000;

/** How long playback must run healthy before a step back up: the
 *  first entry after one step down, each later one after another. */
export const HEALTHY_WINDOWS_MS = [30_000, 60_000, 120_000, 240_000] as const;

/** Seconds of buffer ahead that count as healthy. */
export const HEALTHY_BUFFER_S = 15;

/** Why the level changed, for the log. */
export type QualityReason =
	| 'start'
	| 'stall'
	| 'fragments-failed'
	| 'level-failed'
	| 'healthy'
	| 'pick'
	| 'auto';

/** A change of level for the engine to make. `load` takes effect from
 *  the next segment, keeping what is buffered; `next` replaces the
 *  buffer ahead, so a choice the viewer made shows at once. */
export interface QualityMove {
	readonly level: number;
	readonly from: number | null;
	readonly reason: QualityReason;
	readonly via: 'load' | 'next';
}

export class QualityGovernor {
	constructor(levels: readonly LadderLevel[], target: QualityTarget, pinHeight: number | null) {
		void levels;
		void target;
		void pinHeight;
	}

	/** The level the governor holds the engine at. */
	get level(): number {
		return -1;
	}

	/** Whether the viewer pinned the level. */
	get isPinned(): boolean {
		return false;
	}

	/** Whether the governor holds the engine's level at all: not for a
	 *  stream of one level, and not once every level below a failing
	 *  one has failed too, when the engine's own fallback takes over. */
	get isEngaged(): boolean {
		return false;
	}

	/** Levels the engine could not load, left for the stream. */
	get unavailableLevels(): number[] {
		return [];
	}

	/** The level to start at. */
	start(): QualityMove | null {
		return null;
	}

	/** The viewer seeked. */
	seeking(now: number): void {
		void now;
	}

	/** Playback stalled with nothing buffered. */
	stall(now: number): QualityMove | null {
		void now;
		return null;
	}

	/** The engine gave up on `level`'s segments after its retries. */
	fragmentsFailed(level: number, now: number): QualityMove | null {
		void level;
		void now;
		return null;
	}

	/** The engine could not load `level`'s playlist after its retries. */
	levelFailed(level: number): QualityMove | null {
		void level;
		return null;
	}

	/** A segment of the main rendition was buffered. */
	healthy(now: number, bufferAheadS: number): QualityMove | null {
		void now;
		void bufferAheadS;
		return null;
	}

	/** The viewer chose a level, or "Auto". */
	pick(choice: 'auto' | number, now: number): QualityMove | null {
		void choice;
		void now;
		return null;
	}
}
