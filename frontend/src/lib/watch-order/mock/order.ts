/**
 * Order within a run, as the proposal's section 4 defines it, for the
 * mockups. Link order follows the prequel/sequel links with shortcut
 * links ignored and cycles taken as one block; release order sorts by
 * start date with unknown dates last.
 */
import type { MockEntry, MockRun } from './types';

export type WatchOrder = 'linked' | 'released';

/** Start date ascending, unknown last, then Kitsu id. */
export function byStart(a: MockEntry, b: MockEntry): number {
	if (a.start !== b.start) {
		if (a.start === null) return 1;
		if (b.start === null) return -1;
		return a.start < b.start ? -1 : 1;
	}
	return Number(a.id) - Number(b.id);
}

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

/** Tarjan's components; members of a component larger than one form a cycle. */
function components(ids: readonly string[], out: Map<string, string[]>): Map<string, number> {
	const index = new Map<string, number>();
	const low = new Map<string, number>();
	const stack: string[] = [];
	const onStack = new Set<string>();
	const comp = new Map<string, number>();
	let counter = 0;
	let compCount = 0;
	const visit = (v: string) => {
		index.set(v, counter);
		low.set(v, counter);
		counter++;
		stack.push(v);
		onStack.add(v);
		for (const w of out.get(v) ?? []) {
			if (!index.has(w)) {
				visit(w);
				low.set(v, Math.min(low.get(v)!, low.get(w)!));
			} else if (onStack.has(w)) {
				low.set(v, Math.min(low.get(v)!, index.get(w)!));
			}
		}
		if (low.get(v) === index.get(v)) {
			let w: string;
			do {
				w = stack.pop()!;
				onStack.delete(w);
				comp.set(w, compCount);
			} while (w !== v);
			compCount++;
		}
	};
	for (const v of ids) if (!index.has(v)) visit(v);
	return comp;
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

export interface Placement {
	/** Place counted from the run's first entry, or null where unnumbered. */
	readonly position: number | null;
	/** Run length, only where the order is one line. */
	readonly total: number | null;
	readonly before: readonly MockEntry[];
	readonly after: readonly MockEntry[];
	readonly inCycle: boolean;
}

function releasedLine(run: MockRun): MockEntry[] {
	return [...run.entries].sort(byStart);
}

/**
 * Count along the anchor's own path back to the run's first entry,
 * taking the earliest-starting prequel at a join. A path that stops
 * at a cycle, passes a fork, or ends at an entry other than the run's
 * first is unnumbered: numbering it would give a second "01".
 */
function linkedPosition(graph: RunGraph, id: string, firstId: string): number | null {
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

export function placement(run: MockRun, id: string, order: WatchOrder): Placement {
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

/**
 * A cycle's members in the order its links give: from the earliest
 * member a link enters by, follow each member's earliest unvisited
 * sequel inside the cycle; where that runs out, carry on from the
 * earliest member not yet listed.
 */
function cycleWalk(graph: RunGraph, byDate: readonly MockEntry[]): MockEntry[] {
	const comp = graph.component.get(byDate[0].id);
	const inside = (e: MockEntry) => graph.component.get(e.id) === comp;
	const entered = byDate.filter((e) => graph.prev.get(e.id)!.some((p) => !inside(p)));
	const out: MockEntry[] = [];
	const seen = new Set<string>();
	let cur: MockEntry | undefined = entered[0] ?? byDate[0];
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
export function sequence(run: MockRun, order: WatchOrder): MockEntry[] {
	const line = releasedLine(run);
	if (order === 'released') return line;
	const graph = linkGraph(run);
	const members = new Map<number, MockEntry[]>();
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
	const result: MockEntry[] = [];
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

export type Segment =
	| { readonly kind: 'line'; readonly entries: readonly MockEntry[] }
	| {
			readonly kind: 'fork';
			readonly from: MockEntry;
			readonly branches: readonly (readonly MockEntry[])[];
	  }
	| { readonly kind: 'cycle'; readonly entries: readonly MockEntry[] };

/** One branch from its first entry: the chain that follows it while nothing forks or joins. */
function branchFrom(graph: RunGraph, head: MockEntry, placed: Set<string>): MockEntry[] {
	const chain: MockEntry[] = [];
	let cur: MockEntry | undefined = head;
	while (
		cur &&
		!placed.has(cur.id) &&
		!graph.cycle.has(cur.id) &&
		graph.prev.get(cur.id)!.length === 1
	) {
		chain.push(cur);
		placed.add(cur.id);
		const after: readonly MockEntry[] = graph.next.get(cur.id)!;
		cur = after.length === 1 ? after[0] : undefined;
	}
	return chain;
}

/**
 * The run as the franchise page lays it out: straight stretches, a
 * fork with its branches side by side, and a cycle as one block.
 * Release order is always one straight line.
 */
export function segments(run: MockRun, order: WatchOrder): Segment[] {
	const seq = sequence(run, order);
	if (order === 'released') return [{ kind: 'line', entries: seq }];
	const graph = linkGraph(run);
	const placed = new Set<string>();
	const out: Segment[] = [];
	let line: MockEntry[] = [];
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

export type PathNote =
	| { readonly kind: 'cycle' }
	| { readonly kind: 'branch' }
	| { readonly kind: 'leads-into'; readonly entry: MockEntry }
	| null;

/** Why an unnumbered entry has no number, for the line under its card. */
export function pathNote(run: MockRun, id: string, order: WatchOrder): PathNote {
	if (order === 'released') return null;
	const graph = linkGraph(run);
	if (graph.cycle.has(id)) return { kind: 'cycle' };
	if (linkedPosition(graph, id, run.nameId) !== null) return null;
	const before = graph.prev.get(id)!;
	if (before.length === 1 && graph.next.get(before[0].id)!.length > 1) return { kind: 'branch' };
	const after = graph.next.get(id)!;
	if (before.length === 0 && after.length === 1) return { kind: 'leads-into', entry: after[0] };
	return null;
}

export type RailKey =
	| { readonly kind: 'way'; readonly n: number; readonly count: number }
	| { readonly kind: 'loop'; readonly count: number }
	| { readonly kind: 'leads-into'; readonly entry: MockEntry; readonly position: number | null }
	| null;

export interface RailItem {
	readonly entry: MockEntry;
	readonly position: number | null;
	/** What the card's key line says about its place, when it is not a plain step. */
	readonly key: RailKey;
	/** A hairline before the card: a branch, a loop or a side entry starts here. */
	readonly divider: boolean;
	/** A member of a loop: a rule over its poster joins the loop's cards. */
	readonly inLoop: boolean;
}

/**
 * The run as rail cards, with the structure the flat row would hide:
 * each branch after a fork opens with "Way n of N", a loop opens with
 * its size, and an entry that only leads into a join says so.
 */
export function railItems(run: MockRun, order: WatchOrder): RailItem[] {
	const out: RailItem[] = [];
	const pos = (e: MockEntry) => placement(run, e.id, order).position;
	for (const seg of segments(run, order)) {
		if (seg.kind === 'fork') {
			seg.branches.forEach((branch, b) =>
				branch.forEach((e, i) =>
					out.push({
						entry: e,
						position: pos(e),
						key: i === 0 ? { kind: 'way', n: b + 1, count: seg.branches.length } : null,
						divider: i === 0,
						inLoop: false
					})
				)
			);
		} else if (seg.kind === 'cycle') {
			seg.entries.forEach((e, i) =>
				out.push({
					entry: e,
					position: null,
					key: i === 0 ? { kind: 'loop', count: seg.entries.length } : null,
					divider: i === 0,
					inLoop: true
				})
			);
		} else {
			let afterLoop = out.length > 0 && out.at(-1)!.position === null;
			for (const e of seg.entries) {
				const note = pathNote(run, e.id, order);
				const leads = note?.kind === 'leads-into' ? note : null;
				out.push({
					entry: e,
					position: pos(e),
					key: leads
						? { kind: 'leads-into', entry: leads.entry, position: pos(leads.entry) }
						: null,
					divider: leads !== null || afterLoop,
					inLoop: false
				});
				afterLoop = false;
			}
		}
	}
	return out;
}

/** How many entries share the loop `id` sits in, or 0 outside one. */
export function loopSize(run: MockRun, id: string, order: WatchOrder): number {
	for (const seg of segments(run, order)) {
		if (seg.kind === 'cycle' && seg.entries.some((e) => e.id === id)) return seg.entries.length;
	}
	return 0;
}
