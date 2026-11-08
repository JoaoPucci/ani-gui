/**
 * Why an entry of the run has no number: a branch, a loop, or an
 * entry that only leads into a join. Part of the run's ordering;
 * `./order` gathers the pieces.
 */
import type { WatchOrder } from './by-start';
import { linkGraph } from './graph';
import { linkedPosition } from './place';
import type { WatchEntry, WatchRun } from './types';

export type PathNote =
	| { readonly kind: 'cycle' }
	| { readonly kind: 'branch' }
	| { readonly kind: 'leads-into'; readonly entry: WatchEntry }
	| null;

/** Why an unnumbered entry has no number, for the line under its card. */
export function pathNote(run: WatchRun, id: string, order: WatchOrder): PathNote {
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
