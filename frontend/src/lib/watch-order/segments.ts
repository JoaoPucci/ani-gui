/**
 * The run laid out for the franchise page: straight stretches, forks
 * with their branches side by side, and loops as one block.
 * Part of the run's ordering; `./order` gathers the pieces.
 */
import type { WatchOrder } from './by-start';
import { linkGraph, type RunGraph } from './graph';
import { sequence } from './sequence';
import type { WatchEntry, WatchRun } from './types';

export type Segment =
	| { readonly kind: 'line'; readonly entries: readonly WatchEntry[] }
	| {
			readonly kind: 'fork';
			readonly from: WatchEntry;
			readonly branches: readonly (readonly WatchEntry[])[];
	  }
	| { readonly kind: 'cycle'; readonly entries: readonly WatchEntry[] };

/** One branch from its first entry: the chain that follows it while nothing forks or joins. */
function branchFrom(graph: RunGraph, head: WatchEntry, placed: Set<string>): WatchEntry[] {
	const chain: WatchEntry[] = [];
	let cur: WatchEntry | undefined = head;
	while (
		cur &&
		!placed.has(cur.id) &&
		!graph.cycle.has(cur.id) &&
		graph.prev.get(cur.id)!.length === 1
	) {
		chain.push(cur);
		placed.add(cur.id);
		const after: readonly WatchEntry[] = graph.next.get(cur.id)!;
		cur = after.length === 1 ? after[0] : undefined;
	}
	return chain;
}

/**
 * The run as the franchise page lays it out: straight stretches, a
 * fork with its branches side by side, and a cycle as one block.
 * Release order is always one straight line.
 */
export function segments(run: WatchRun, order: WatchOrder): Segment[] {
	const seq = sequence(run, order);
	if (order === 'released') return [{ kind: 'line', entries: seq }];
	const graph = linkGraph(run);
	const placed = new Set<string>();
	const out: Segment[] = [];
	let line: WatchEntry[] = [];
	const flush = () => {
		if (line.length > 0) out.push({ kind: 'line', entries: line });
		line = [];
	};
	for (const e of seq) {
		if (placed.has(e.id)) continue;
		if (graph.cycle.has(e.id)) {
			flush();
			const comp = graph.component.get(e.id);
			const members = seq.filter((x) => graph.component.get(x.id) === comp);
			for (const x of members) placed.add(x.id);
			out.push({ kind: 'cycle', entries: members });
			continue;
		}
		placed.add(e.id);
		line.push(e);
		const after = graph.next.get(e.id)!;
		if (after.length > 1) {
			flush();
			const branches = after.map((n) => branchFrom(graph, n, placed)).filter((b) => b.length > 0);
			// Branches that run straight into a loop are the loop's to
			// show: a fork with nothing left in it is not drawn.
			if (branches.length > 0) out.push({ kind: 'fork', from: e, branches });
		}
	}
	flush();
	return out;
}

/** How many entries share the loop `id` sits in, or 0 outside one. */
export function loopSize(run: WatchRun, id: string, order: WatchOrder): number {
	for (const seg of segments(run, order)) {
		if (seg.kind === 'cycle' && seg.entries.some((e) => e.id === id)) return seg.entries.length;
	}
	return 0;
}
