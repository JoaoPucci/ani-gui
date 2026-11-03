/**
 * The show page's strip from the franchise payload: the run the
 * ordering works on, and what the strip shows for one id, or no strip
 * at all.
 */
import type { WatchOrder } from './by-start';
import type { StripView } from './strip-view';
import type { FranchiseData, WatchRun } from './types';

/**
 * The payload as a run: links and parents kept only where they name
 * members, and the run's first entry found. Null for an empty run.
 */
export function runOf(franchise: FranchiseData): WatchRun | null {
	void franchise;
	return null;
}

export interface StripModel {
	readonly run: WatchRun;
	readonly view: StripView;
}

/**
 * What the strip shows on `id`'s page: its place in the run, or a side
 * story's parents. Null (no strip) when the fetch failed, the payload
 * answers another id, the run is `id` alone with no parents, or `id`
 * is neither a member nor a side story with parents in the run.
 */
export function stripFor(
	franchise: FranchiseData | null,
	id: string,
	order: WatchOrder
): StripModel | null {
	void franchise;
	void id;
	void order;
	return null;
}
