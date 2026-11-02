/**
 * Order within a run, as the proposal's section 4 defines it, for the
 * mockups. Signatures only: every reading is empty until the rules
 * land.
 */
import type { MockEntry, MockRun } from './types';

export type WatchOrder = 'linked' | 'released';

export function byStart(a: MockEntry, b: MockEntry): number {
	void a;
	void b;
	return 0;
}

export interface RunGraph {
	readonly prev: ReadonlyMap<string, readonly MockEntry[]>;
	readonly next: ReadonlyMap<string, readonly MockEntry[]>;
	readonly cycle: ReadonlySet<string>;
	readonly component: ReadonlyMap<string, number>;
}

export function linkGraph(run: MockRun): RunGraph {
	void run;
	return { prev: new Map(), next: new Map(), cycle: new Set(), component: new Map() };
}

export interface Placement {
	readonly position: number | null;
	readonly total: number | null;
	readonly before: readonly MockEntry[];
	readonly after: readonly MockEntry[];
	readonly inCycle: boolean;
}

export function placement(run: MockRun, id: string, order: WatchOrder): Placement {
	void run;
	void id;
	void order;
	return { position: null, total: null, before: [], after: [], inCycle: false };
}

export function sequence(run: MockRun, order: WatchOrder): MockEntry[] {
	void run;
	void order;
	return [];
}

export type Segment =
	| { readonly kind: 'line'; readonly entries: readonly MockEntry[] }
	| {
			readonly kind: 'fork';
			readonly from: MockEntry;
			readonly branches: readonly (readonly MockEntry[])[];
	  }
	| { readonly kind: 'cycle'; readonly entries: readonly MockEntry[] };

export function segments(run: MockRun, order: WatchOrder): Segment[] {
	void run;
	void order;
	return [];
}

export type PathNote =
	| { readonly kind: 'cycle' }
	| { readonly kind: 'branch' }
	| { readonly kind: 'leads-into'; readonly entry: MockEntry }
	| null;

export function pathNote(run: MockRun, id: string, order: WatchOrder): PathNote {
	void run;
	void id;
	void order;
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
	readonly key: RailKey;
	readonly divider: boolean;
	readonly inLoop: boolean;
}

export function railItems(run: MockRun, order: WatchOrder): RailItem[] {
	void run;
	void order;
	return [];
}

export function loopSize(run: MockRun, id: string, order: WatchOrder): number {
	void run;
	void id;
	void order;
	return 0;
}
