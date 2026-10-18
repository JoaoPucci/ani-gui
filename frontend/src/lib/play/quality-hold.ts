/**
 * The level the quality governor holds, and the moves that do not
 * depend on time: the start, a level the engine cannot load, and the
 * viewer's choice. `quality-governor.ts` adds the moves that do —
 * stalls, failing segments and the climb back — on top of this.
 */

import {
	ladderOrder,
	isLoadable,
	neighbourLevel,
	targetLevel,
	type LadderLevel,
	type QualityPinChoice,
	type QualityTarget
} from './quality-ladder';
import { demotePin, startPin } from './quality-pinlevel';
import { remapHold } from './quality-remap';

/** Why the level changed, for the log. */
export type QualityReason =
	| 'start'
	| 'stall'
	| 'fragments-failed'
	| 'level-failed'
	| 'healthy'
	| 'pick'
	| 'auto'
	| 'followed';

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
	protected ladder: number[];
	/** The setting's level: auto never climbs above it. */
	protected ceiling: number;
	protected current: number;
	/** The level the viewer pinned, or null while the choice is Auto. */
	protected pinLevel: number | null;
	protected engaged: boolean;
	protected unavailable = new Set<number>();

	constructor(
		levels: readonly LadderLevel[],
		private readonly target: QualityTarget,
		pin: QualityPinChoice | null
	) {
		this.ladder = ladderOrder(levels);
		this.ceiling = targetLevel(target, levels);
		// A pin with no level at or below it starts the stream as Auto.
		this.pinLevel = startPin(pin, levels);
		this.current = this.pinLevel ?? this.ceiling;
		this.engaged = levels.length > 1;
	}

	/** The level the governor holds the engine at. */
	get level(): number {
		return this.current;
	}

	/** The level the viewer pinned — the choice in force, which the hold
	 *  may sit below after a failure — or null while the choice is Auto. */
	get pinnedLevel(): number | null {
		return this.pinLevel;
	}

	/** Whether the viewer pinned a level. */
	get isPinned(): boolean {
		return this.pinLevel !== null;
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

	/** hls.js removed levels: every index held is carried to the levels
	 *  left (`quality-remap.ts`). */
	remap(mapping: readonly number[], levels: readonly LadderLevel[], now: number): void {
		void now;
		const held = {
			ladder: this.ladder,
			ceiling: this.ceiling,
			current: this.current,
			engaged: this.engaged,
			pinLevel: this.pinLevel,
			unavailable: this.unavailable
		};
		const next = remapHold(held, mapping, levels, this.target);
		this.ladder = next.ladder;
		this.ceiling = next.ceiling;
		this.current = next.current;
		this.engaged = next.engaged;
		this.pinLevel = next.pinLevel;
		this.unavailable = next.unavailable;
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
	levelFailed(level: number, now?: number): QualityMove | null {
		this.unavailable.add(level);
		// A pin on a level that cannot load moves down to one that can;
		// with none, it is let go.
		this.pinLevel = demotePin(this.ladder, this.unavailable, this.pinLevel);
		if (!this.engaged || level !== this.current) return null;
		// The nearest loadable level below, else above — which is at most
		// the pin, since the pin is loadable and above the hold.
		const next = this.neighbour(level, -1) ?? this.neighbour(level, 1);
		if (next === null) return this.release();
		this.settled(now);
		return this.move(next, 'level-failed', 'load');
	}

	/** The viewer chose a level, or "Auto". */
	pick(choice: 'auto' | number, now: number): QualityMove | null {
		if (this.ladder.length < 2) return null;
		if (choice === 'auto') return this.auto(now);
		if (!isLoadable(this.ladder, this.unavailable, choice)) return null;
		this.pinLevel = choice;
		this.engaged = true;
		return choice === this.current ? null : this.move(choice, 'pick', 'next');
	}

	/** The choice goes back to the engine, and the stream's pin with it:
	 *  the menu must not show a level the engine is not held at. The
	 *  run's pin, by height, stays for the next stream. */
	protected release(): null {
		this.engaged = false;
		this.pinLevel = null;
		return null;
	}

	/** The level to put the engine back at after hls.js resumed its own
	 *  choice — as it does after a level's error — or null when the
	 *  governor holds nothing. */
	reclaim(engineChoosing: boolean): number | null {
		return this.engaged && engineChoosing ? this.current : null;
	}

	/** The choice went back to auto at `now`. */
	protected abstract restarted(now: number): void;

	/** The level moved, at `now`, to one that has proven nothing yet. */
	protected abstract settled(now: number | undefined): void;

	protected move(level: number, reason: QualityReason, via: QualityMove['via']): QualityMove {
		const from = this.current;
		this.current = level;
		return { level, from, reason, via };
	}

	protected neighbour(level: number, direction: 1 | -1): number | null {
		return neighbourLevel(this.ladder, this.unavailable, level, direction);
	}

	private auto(now: number): QualityMove | null {
		this.pinLevel = null;
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
