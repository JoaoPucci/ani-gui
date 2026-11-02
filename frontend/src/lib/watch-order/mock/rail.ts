/**
 * The run as rail cards, and the keys that say why an entry has no
 * number: a branch, a loop, or an entry that only leads into a join.
 * Part of the mockups' ordering; `./order` gathers the pieces.
 */
import type { WatchOrder } from './by-start';
import { linkGraph } from './graph';
import { linkedPosition, placement } from './place';
import { segments } from './segments';
import type { MockEntry, MockRun } from './types';

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
