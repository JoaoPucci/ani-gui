/**
 * The quality state carried across hls.js removing levels.
 *
 * On a mixed-codec master hls.js removes the variants the browser
 * refuses (level-controller `removeLevel`): the levels left are
 * re-indexed and LEVELS_UPDATED carries the same level objects, fewer
 * of them. Everything the control keys by level index — the level
 * held, the ceiling, the levels left unavailable, the heights
 * learned, the level playing — is carried to the new indexes by level
 * identity here. The removal happens inside an error's own dispatch,
 * before the app's ERROR listener, so that error's `nextAutoLevel`
 * still names the old list; `RemapWindow` reads it in the new one.
 */

import { ladderOrder, neighbourLevel, targetLevel } from './quality-ladder';
import type { LadderLevel, QualityTarget } from './quality-ladder';
import type { QualityReport } from './quality-report';

/** Each old index's new index, -1 for a level removed. */
export function levelMapping(before: readonly unknown[], after: readonly unknown[]): number[] {
	return before.map((level) => after.indexOf(level));
}

/** An index carried across, or null when its level was removed. */
export function remapIndex(index: number | null, mapping: readonly number[]): number | null {
	const next = index === null ? -1 : (mapping[index] ?? -1);
	return next >= 0 ? next : null;
}

/** A map keyed by level index, carried across; removed levels drop. */
export function remapKeys<V>(
	map: ReadonlyMap<number, V>,
	mapping: readonly number[]
): Map<number, V> {
	const out = new Map<number, V>();
	for (const [index, value] of map) {
		const next = remapIndex(index, mapping);
		if (next !== null) out.set(next, value);
	}
	return out;
}

/** What the governor holds, as the remap reads and rebuilds it. */
export interface HoldState {
	ladder: number[];
	ceiling: number;
	current: number;
	engaged: boolean;
	unavailable: Set<number>;
}

/** The hold carried to the levels left. A ceiling removed is the
 *  setting's level among them; a held level removed moves to the
 *  ceiling, or the nearest loadable level to it. */
export function remapHold(
	state: HoldState,
	mapping: readonly number[],
	levels: readonly LadderLevel[],
	target: QualityTarget
): HoldState {
	const ladder = ladderOrder(levels);
	const unavailable = new Set(
		remapKeys(new Map([...state.unavailable].map((i) => [i, i])), mapping).keys()
	);
	const ceiling = remapIndex(state.ceiling, mapping) ?? targetLevel(target, levels);
	const current =
		remapIndex(state.current, mapping) ?? nearestLoadable(ladder, unavailable, ceiling);
	return { ladder, ceiling, current, unavailable, engaged: state.engaged && levels.length > 1 };
}

function nearestLoadable(ladder: number[], unavailable: Set<number>, level: number): number {
	if (!unavailable.has(level)) return level;
	return (
		neighbourLevel(ladder, unavailable, level, -1) ??
		neighbourLevel(ladder, unavailable, level, 1) ??
		level
	);
}

/** A removal, for the rest of its dispatch. hls.js removes levels one
 *  at a time, each with its own LEVELS_UPDATED, inside the error that
 *  caused them, which names its switch in the list the dispatch began
 *  with: the window maps from that list to the levels now. */
export class RemapWindow {
	private origin: readonly unknown[] | null = null;
	private mapping: readonly number[] | null = null;
	private closed = false;

	/** Levels went from `before` to `after`. */
	opened(before: readonly unknown[], after: readonly unknown[]): void {
		if (!this.origin) {
			this.origin = before;
			queueMicrotask(() => {
				this.origin = null;
				this.mapping = null;
			});
		}
		this.mapping = levelMapping(this.origin, after);
	}

	/** A report read in the levels as they are now. A switch is read
	 *  from the list the error named; a segment failure inside a
	 *  removal is hls.js's own, resolved by the removal, and moves
	 *  nothing — the hold has been re-seated on a level that did not
	 *  fail. */
	read(report: QualityReport | null): QualityReport | null {
		if (!this.mapping || !report || report.kind === 'stall') return report;
		if (report.kind !== 'followed') return null;
		const level = remapIndex(report.level, this.mapping);
		return level === null ? null : { kind: 'followed', level };
	}

	/** The stream retired: nothing scheduled acts on it any more. */
	close(): void {
		this.closed = true;
	}

	get isClosed(): boolean {
		return this.closed;
	}
}
