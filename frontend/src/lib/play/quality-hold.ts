/**
 * The level the quality governor holds, and the moves that do not
 * depend on time: the start, a level the engine cannot load, and the
 * viewer's choice. `quality-governor.ts` adds the moves that do —
 * stalls, failing segments and the climb back — on top of this.
 */

import {
	ladderOrder,
	neighbourLevel,
	targetLevel,
	type LadderLevel,
	type QualityPinChoice,
	type QualityTarget
} from './quality-ladder';

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

export abstract class QualityHold {
	/** Level indices, lowest first. */
	protected readonly ladder: number[];
	/** The setting's level: auto never climbs above it. */
	protected readonly ceiling: number;
	protected current: number;
	protected pinned: boolean;
	protected engaged: boolean;
	protected readonly unavailable = new Set<number>();

	constructor(levels: readonly LadderLevel[], target: QualityTarget, pin: QualityPinChoice | null) {
		this.ladder = ladderOrder(levels);
		this.ceiling = targetLevel(target, levels);
		this.pinned = typeof pin === 'number';
		this.current =
			typeof pin === 'number' ? targetLevel({ kind: 'height', height: pin }, levels) : this.ceiling;
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

	/** The viewer chose a level, or "Auto". */
	pick(choice: 'auto' | number, now: number): QualityMove | null {
		if (this.ladder.length < 2) return null;
		if (choice === 'auto') return this.auto(now);
		if (!this.ladder.includes(choice) || this.unavailable.has(choice)) return null;
		this.pinned = true;
		this.engaged = true;
		return choice === this.current ? null : this.move(choice, 'pick', 'next');
	}

	/** The choice went back to auto at `now`. */
	protected abstract restarted(now: number): void;

	protected move(level: number, reason: QualityReason, via: QualityMove['via']): QualityMove {
		const from = this.current;
		this.current = level;
		return { level, from, reason, via };
	}

	protected neighbour(level: number, direction: 1 | -1): number | null {
		return neighbourLevel(this.ladder, this.unavailable, level, direction);
	}

	private auto(now: number): QualityMove | null {
		this.pinned = false;
		this.restarted(now);
		const below = this.unavailable.has(this.ceiling)
			? this.neighbour(this.ceiling, -1)
			: this.ceiling;
		const top = below ?? this.neighbour(this.ceiling, 1);
		if (top === null) return null;
		this.engaged = true;
		return top === this.current ? null : this.move(top, 'auto', 'next');
	}
}
