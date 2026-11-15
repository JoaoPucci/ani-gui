/**
 * The franchise page as the show page reaches it: what it lists beside
 * the show, and the lone link where the show has no strip to carry it.
 */
import type { WatchOrderData } from './loader';
import { hasStrip, runOf } from './strip-model';
import type { WatchRun } from './types';

/**
 * Every title the franchise page lists for `run` but `id` itself: the
 * run, its other versions, films, side stories and recaps.
 */
export function restOfFranchise(run: WatchRun, id: string): ReadonlySet<string> {
	const all = [...run.entries, ...run.versions, ...run.films, ...run.sides, ...run.recaps];
	return new Set(all.map((e) => e.id).filter((x) => x !== id));
}

/**
 * The way to the franchise page where the show has no strip to carry
 * it: the run the link's stack draws from, whether it was read whole,
 * and what "Similar titles" leaves out because that page lists it.
 */
export interface FranchiseLink {
	readonly run: WatchRun;
	/** The main-story entry the link's poster stack is fronted by. */
	readonly front: string;
	/** Whether the franchise was read whole: see `StripModel.complete`. */
	readonly complete: boolean;
	/** Ids "Similar titles" leaves out: the rest of the franchise. */
	readonly similarHidden: ReadonlySet<string>;
}

/**
 * The franchise link the page shows on its own: where `id` has no
 * strip (it is its run alone, or a film, version, recap or side story
 * outside the run with no parent story in it) but the franchise page
 * would list other titles, such as films, other versions, side
 * stories, recaps or the run itself. Null where the strip shows, which
 * carries the link itself, and where the franchise page would list the
 * show alone, read whole or in part.
 */
export function franchiseLinkOnly(data: WatchOrderData, id: string): FranchiseLink | null {
	if (data.franchise === null || data.franchise.anchor !== id) return null;
	const run = runOf(data.franchise);
	if (run === null || hasStrip(run, id)) return null;
	const rest = restOfFranchise(run, id);
	if (rest.size === 0) return null;
	return {
		run,
		front: data.franchise.name_from,
		complete: data.franchise.complete,
		similarHidden: rest
	};
}
