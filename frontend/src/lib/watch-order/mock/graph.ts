/**
 * The run's link graph: prequel/sequel links between members, with
 * shortcut links (ones a longer path already covers) dropped, and the
 * members that form a cycle.
 * Part of the mockups' ordering; `./order` gathers the pieces.
 */
import { byStart } from './by-start';
import { components } from './components';
import type { MockEntry, MockRun } from './types';

export interface RunGraph {
	/** Kept links, each list in start order. */
	readonly prev: ReadonlyMap<string, readonly MockEntry[]>;
	readonly next: ReadonlyMap<string, readonly MockEntry[]>;
	/** Members that lead round to themselves. */
	readonly cycle: ReadonlySet<string>;
	/** Strongly connected component per member. */
	readonly component: ReadonlyMap<string, number>;
}

function adjacency(ids: readonly string[], links: readonly (readonly [string, string])[]) {
	const out = new Map<string, string[]>(ids.map((i) => [i, []]));
	for (const [a, b] of links) out.get(a)?.push(b);
	return out;
}

/** Whether `to` is reachable from `from` over component links other than the direct one. */
function reachableAround(compOut: Map<number, Set<number>>, from: number, to: number): boolean {
	const seen = new Set<number>();
	const queue = [...(compOut.get(from) ?? [])].filter((c) => c !== to);
	while (queue.length > 0) {
		const c = queue.shift()!;
		if (c === to) return true;
		if (seen.has(c)) continue;
		seen.add(c);
		queue.push(...(compOut.get(c) ?? []));
	}
	return false;
}

export function linkGraph(run: MockRun): RunGraph {
	const ids = run.entries.map((e) => e.id);
	const byId = new Map(run.entries.map((e) => [e.id, e]));
	const comp = components(ids, adjacency(ids, run.links));
	const sizes = new Map<number, number>();
	for (const c of comp.values()) sizes.set(c, (sizes.get(c) ?? 0) + 1);
	const compOut = new Map<number, Set<number>>();
	for (const [a, b] of run.links) {
		const ca = comp.get(a)!;
		const cb = comp.get(b)!;
		if (ca !== cb) compOut.set(ca, (compOut.get(ca) ?? new Set()).add(cb));
	}
	const kept = run.links.filter(([a, b]) => {
		const ca = comp.get(a)!;
		const cb = comp.get(b)!;
		return ca === cb || !reachableAround(compOut, ca, cb);
	});
	const prev = new Map<string, MockEntry[]>(ids.map((i) => [i, []]));
	const next = new Map<string, MockEntry[]>(ids.map((i) => [i, []]));
	for (const [a, b] of kept) {
		next.get(a)!.push(byId.get(b)!);
		prev.get(b)!.push(byId.get(a)!);
	}
	for (const list of [...prev.values(), ...next.values()]) list.sort(byStart);
	const cycle = new Set(ids.filter((i) => sizes.get(comp.get(i)!)! > 1));
	return { prev, next, cycle, component: comp };
}
