/**
 * The run as one list: release order, or the links' order with each
 * cycle walked along its links.
 * Part of the run's ordering; `./order` gathers the pieces.
 */
import { byStart, type WatchOrder } from './by-start';
import { linkGraph, type RunGraph } from './graph';
import { releasedLine } from './place';
import type { WatchEntry, WatchRun } from './types';

/**
 * A cycle's members in the order its links give: from the earliest
 * member a link enters by, follow each member's earliest unvisited
 * sequel inside the cycle; where that runs out, carry on from the
 * earliest member not yet listed.
 */
function cycleWalk(graph: RunGraph, byDate: readonly WatchEntry[]): WatchEntry[] {
	const comp = graph.component.get(byDate[0].id);
	const inside = (e: WatchEntry) => graph.component.get(e.id) === comp;
	const entered = byDate.filter((e) => graph.prev.get(e.id)!.some((p) => !inside(p)));
	const out: WatchEntry[] = [];
	const seen = new Set<string>();
	let cur: WatchEntry | undefined = entered[0] ?? byDate[0];
	while (out.length < byDate.length) {
		cur ??= byDate.find((e) => !seen.has(e.id))!;
		out.push(cur);
		seen.add(cur.id);
		cur = graph.next.get(cur.id)!.find((e) => inside(e) && !seen.has(e.id));
	}
	return out;
}

/**
 * The run as one list for a rail or the franchise page: release order,
 * or the links' order — components in topological order, the earliest
 * start first among those ready, a cycle's members along its links.
 */
export function sequence(run: WatchRun, order: WatchOrder): WatchEntry[] {
	const line = releasedLine(run);
	if (order === 'released') return line;
	const graph = linkGraph(run);
	const members = new Map<number, WatchEntry[]>();
	for (const e of line) {
		const c = graph.component.get(e.id)!;
		members.set(c, [...(members.get(c) ?? []), e]);
	}
	for (const [c, list] of members) if (list.length > 1) members.set(c, cycleWalk(graph, list));
	const indegree = new Map<number, number>([...members.keys()].map((c) => [c, 0]));
	const edges = new Map<number, Set<number>>();
	for (const [id, after] of graph.next) {
		const ca = graph.component.get(id)!;
		for (const e of after) {
			const cb = graph.component.get(e.id)!;
			if (ca === cb || edges.get(ca)?.has(cb)) continue;
			edges.set(ca, (edges.get(ca) ?? new Set()).add(cb));
			indegree.set(cb, indegree.get(cb)! + 1);
		}
	}
	const first = (c: number) => members.get(c)![0];
	const ready = [...members.keys()].filter((c) => indegree.get(c) === 0);
	const result: WatchEntry[] = [];
	while (ready.length > 0) {
		ready.sort((a, b) => byStart(first(a), first(b)));
		const c = ready.shift()!;
		result.push(...members.get(c)!);
		for (const d of edges.get(c) ?? []) {
			indegree.set(d, indegree.get(d)! - 1);
			if (indegree.get(d) === 0) ready.push(d);
		}
	}
	return result;
}
