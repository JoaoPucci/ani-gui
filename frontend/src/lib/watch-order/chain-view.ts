/**
 * A page outside the main story, drawn on a chain (the backend's
 * `chain`: the franchise's entries outside the main story joined to the
 * page, or, for a side story, to its parent, and the main-story titles
 * it meets, its seams). The backend decides the chain; these only
 * reshape it for the strip's readings.
 */
import type { FranchiseData, WatchEntry } from './types';

/** A main-story title beside a chain's own: the one it comes from, or
 *  the one it leads into. */
export type Seam = 'from' | 'into';

/**
 * The franchise as the page's strip draws it: the chain's titles and
 * links as the run, and the sections without them. Null on a page with
 * no chain.
 */
export function chainFranchise(f: FranchiseData): FranchiseData | null {
	const chain = f.chain;
	if (!chain) return null;
	const inChain = new Set(chain.entries.map((e) => e.id));
	const out = (list: readonly WatchEntry[]) => list.filter((e) => !inChain.has(e.id));
	return {
		anchor: f.anchor,
		entries: chain.entries,
		links: chain.links,
		versions: out(f.versions),
		films: out(f.films),
		sides: out(f.sides),
		recaps: out(f.recaps),
		parents: f.parents,
		complete: f.complete
	};
}

/** The chain's own titles alone, as a run: what a place on it is counted over. */
export function ownFranchise(f: FranchiseData): FranchiseData | null {
	const drawn = chainFranchise(f);
	if (!drawn || !f.chain) return null;
	const seams = new Set(f.chain.seams);
	const entries = drawn.entries.filter((e) => !seams.has(e.id));
	const own = new Set(entries.map((e) => e.id));
	const links = drawn.links.filter(([a, b]) => own.has(a) && own.has(b));
	return { ...drawn, entries, links };
}

/** The seam a column shows: a seam standing in it alone, else none. */
export function seamOf(
	list: readonly WatchEntry[],
	seams: readonly string[],
	side: 'before' | 'after'
): Seam | null {
	if (list.length !== 1 || !seams.includes(list[0].id)) return null;
	return side === 'before' ? 'from' : 'into';
}
