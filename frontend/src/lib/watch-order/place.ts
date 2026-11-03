/**
 * One entry's place in its run: its number, the run's length where the
 * order is one line, and the entries just before and after it.
 * Part of the run's ordering; `./order` gathers the pieces.
 */
import { byStart, type WatchOrder } from './by-start';
import { linkGraph, type RunGraph } from './graph';
import type { WatchEntry, WatchRun } from './types';

export interface Placement {
	/** Place counted from the run's first entry, or null where unnumbered. */
	readonly position: number | null;
	/** Run length, only where the order is one line. */
	readonly total: number | null;
	readonly before: readonly WatchEntry[];
	readonly after: readonly WatchEntry[];
	readonly inCycle: boolean;
}

export function releasedLine(run: WatchRun): WatchEntry[] {
	return [...run.entries].sort(byStart);
}

/**
 * Count along the anchor's own path back to the run's first entry,
 * taking the earliest-starting prequel at a join. A path that stops
 * at a cycle, passes a fork, or ends at an entry other than the run's
 * first is unnumbered: numbering it would give a second "01".
 */
export function linkedPosition(graph: RunGraph, id: string, firstId: string): number | null {
	let cur = id;
	let steps = 0;
	for (;;) {
		if (graph.cycle.has(cur)) return null;
		const before = graph.prev.get(cur)!;
		if (before.length === 0) return cur === firstId ? steps + 1 : null;
		const pick = before[0];
		if (graph.next.get(pick.id)!.length > 1) return null;
		cur = pick.id;
		steps++;
	}
}

function isLine(graph: RunGraph): boolean {
	if (graph.cycle.size > 0) return false;
	for (const list of [...graph.prev.values(), ...graph.next.values()]) {
		if (list.length > 1) return false;
	}
	return true;
}

export function placement(run: WatchRun, id: string, order: WatchOrder): Placement {
	if (order === 'released') {
		const line = releasedLine(run);
		const at = line.findIndex((e) => e.id === id);
		return {
			position: at + 1,
			total: line.length,
			before: line.slice(Math.max(0, at - 1), at),
			after: line.slice(at + 1, at + 2),
			inCycle: false
		};
	}
	const graph = linkGraph(run);
	return {
		position: linkedPosition(graph, id, run.nameId),
		total: isLine(graph) ? run.entries.length : null,
		before: graph.prev.get(id)!,
		after: graph.next.get(id)!,
		inCycle: graph.cycle.has(id)
	};
}
