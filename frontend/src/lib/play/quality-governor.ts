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

import { ladderOrder, targetLevel, type LadderLevel, type QualityTarget } from './quality-ladder';

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
	/** Level indices, lowest first. */
	private readonly ladder: number[];
	/** The setting's level: auto never climbs above it. */
	private readonly ceiling: number;
	private current: number;
	private pinned: boolean;
	private engaged: boolean;
	private readonly unavailable = new Set<number>();
	private stepDowns = 0;
	private lastStepDownAt = -Infinity;
	private lastChangeAt = -Infinity;
	private lastSeekAt = -Infinity;

	constructor(levels: readonly LadderLevel[], target: QualityTarget, pinHeight: number | null) {
		this.ladder = ladderOrder(levels);
		this.ceiling = targetLevel(target, levels);
		this.pinned = pinHeight !== null;
		this.current =
			pinHeight === null
				? this.ceiling
				: targetLevel({ kind: 'height', height: pinHeight }, levels);
		this.engaged = levels.length > 1;
	}

	/** The level the governor holds the engine at. */
	get level(): number {
		return this.current;
	}

	/** Whether the viewer pinned the level. */
	get isPinned(): boolean {
		return this.pinned;
	}

	/** Whether the governor holds the engine's level at all: not for a
	 *  stream of one level, and not once every level below a failing
	 *  one has failed too, when the engine's own fallback takes over. */
	get isEngaged(): boolean {
		return this.engaged;
	}

	/** Levels the engine could not load, left for the stream. */
	get unavailableLevels(): number[] {
		return [...this.unavailable];
	}

	/** The level to start at. */
	start(): QualityMove | null {
		if (!this.engaged) return null;
		return { level: this.current, from: null, reason: 'start', via: 'load' };
	}

	/** The viewer seeked. */
	seeking(now: number): void {
		this.lastSeekAt = now;
	}

	/** Playback stalled with nothing buffered. */
	stall(now: number): QualityMove | null {
		if (!this.engaged || this.pinned) return null;
		if (now - this.lastSeekAt < SEEK_GRACE_MS) return null;
		if (now - this.lastStepDownAt < STEP_DOWN_COOLDOWN_MS) return null;
		const lower = this.neighbour(this.current, -1);
		return lower === null ? null : this.stepDown(lower, now, 'stall');
	}

	/** The engine gave up on `level`'s segments after its retries. Not
	 *  a stall to weigh: the level just failed, so the step is taken at
	 *  once, pinned or not — and with nothing below it, the engine's own
	 *  fallback is left to try what it can. */
	fragmentsFailed(level: number, now: number): QualityMove | null {
		if (!this.engaged || level !== this.current) return null;
		const lower = this.neighbour(this.current, -1);
		if (lower === null) {
			this.engaged = false;
			return null;
		}
		return this.stepDown(lower, now, 'fragments-failed');
	}

	/** The engine could not load `level`'s playlist after its retries:
	 *  the level is left for the stream, and if it is the one held the
	 *  governor moves to the nearest level below — above, when it was
	 *  the lowest. */
	levelFailed(level: number): QualityMove | null {
		this.unavailable.add(level);
		if (!this.engaged || level !== this.current) return null;
		const next = this.neighbour(level, -1) ?? this.neighbour(level, 1);
		if (next === null) {
			this.engaged = false;
			return null;
		}
		return this.move(next, 'level-failed', 'load');
	}

	/** A segment of the main rendition was buffered. */
	healthy(now: number, bufferAheadS: number): QualityMove | null {
		if (!this.engaged || this.pinned || this.stepDowns === 0) return null;
		if (bufferAheadS < HEALTHY_BUFFER_S) return null;
		const window = HEALTHY_WINDOWS_MS[Math.min(this.stepDowns, HEALTHY_WINDOWS_MS.length) - 1];
		if (now - this.lastChangeAt < window) return null;
		const higher = this.neighbour(this.current, 1);
		if (higher === null || this.rank(higher) > this.rank(this.ceiling)) return null;
		this.lastChangeAt = now;
		return this.move(higher, 'healthy', 'load');
	}

	/** The viewer chose a level, or "Auto". */
	pick(choice: 'auto' | number, now: number): QualityMove | null {
		if (this.ladder.length < 2) return null;
		if (choice === 'auto') return this.auto(now);
		if (!this.ladder.includes(choice) || this.unavailable.has(choice)) return null;
		this.pinned = true;
		this.engaged = true;
		return choice === this.current ? null : this.move(choice, 'pick', 'next');
	}

	private auto(now: number): QualityMove | null {
		this.pinned = false;
		this.stepDowns = 0;
		this.lastChangeAt = now;
		const top = this.allowedAtOrBelow(this.ceiling) ?? this.neighbour(this.ceiling, 1);
		if (top === null) return null;
		this.engaged = true;
		return top === this.current ? null : this.move(top, 'auto', 'next');
	}

	private stepDown(level: number, now: number, reason: QualityReason): QualityMove {
		this.stepDowns += 1;
		this.lastStepDownAt = now;
		this.lastChangeAt = now;
		return this.move(level, reason, 'load');
	}

	private move(level: number, reason: QualityReason, via: QualityMove['via']): QualityMove {
		const from = this.current;
		this.current = level;
		return { level, from, reason, via };
	}

	private rank(level: number): number {
		return this.ladder.indexOf(level);
	}

	/** The nearest loadable level above (`1`) or below (`-1`) `level`. */
	private neighbour(level: number, direction: 1 | -1): number | null {
		for (let r = this.rank(level) + direction; r >= 0 && r < this.ladder.length; r += direction) {
			if (!this.unavailable.has(this.ladder[r])) return this.ladder[r];
		}
		return null;
	}

	private allowedAtOrBelow(level: number): number | null {
		return this.unavailable.has(level) ? this.neighbour(level, -1) : level;
	}
}
