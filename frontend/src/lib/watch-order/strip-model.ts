/**
 * The show page's strip from the franchise payload: the run the
 * ordering works on, and what the strip shows for one id, or no strip
 * at all.
 */
import type { WatchOrder } from './by-start';
import { nameId } from './name-id';
import { stripView, type StripView } from './strip-view';
import type { FranchiseData, RunLink, WatchEntry, WatchRun } from './types';

/** `entries` with each id once, the first listing kept. */
function uniqueEntries(entries: readonly WatchEntry[]): WatchEntry[] {
	const seen = new Set<string>();
	return entries.filter((e) => !seen.has(e.id) && seen.add(e.id));
}

/** Links between two different members, each pair once. */
function memberLinks(links: readonly RunLink[], members: ReadonlySet<string>): RunLink[] {
	const seen = new Set<string>();
	return links.filter(([a, b]) => {
		const key = `${a}\n${b}`;
		const keep = a !== b && members.has(a) && members.has(b) && !seen.has(key);
		seen.add(key);
		return keep;
	});
}

/** Each side story's parents among the members, dropping a side story left with none. */
function memberParents(
	parents: FranchiseData['parents'],
	members: ReadonlySet<string>
): Record<string, readonly string[]> {
	const out: Record<string, readonly string[]> = {};
	for (const [side, list] of Object.entries(parents)) {
		const kept = [...new Set(list)].filter((p) => members.has(p));
		if (kept.length > 0) out[side] = kept;
	}
	return out;
}

/**
 * The payload as a run: links and parents kept only where they name
 * members, and the run's first entry found. Null for an empty run.
 */
export function runOf(franchise: FranchiseData): WatchRun | null {
	const entries = uniqueEntries(franchise.entries);
	const members = new Set(entries.map((e) => e.id));
	const links = memberLinks(franchise.links, members);
	const first = nameId(entries, links);
	if (first === null) return null;
	return {
		nameId: first,
		entries,
		links,
		versions: franchise.versions,
		films: franchise.films,
		sides: franchise.sides,
		recaps: franchise.recaps,
		parents: memberParents(franchise.parents, members)
	};
}

export interface StripModel {
	readonly run: WatchRun;
	readonly view: StripView;
}

/** Whether `id`'s strip would show anything: a run beyond `id` itself, or parent stories. */
function hasStrip(run: WatchRun, id: string): boolean {
	if (run.parents[id] !== undefined) return true;
	return run.entries.length > 1 && run.entries.some((e) => e.id === id);
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
	if (franchise === null || franchise.anchor !== id) return null;
	const run = runOf(franchise);
	if (run === null || !hasStrip(run, id)) return null;
	return { run, view: stripView(run, id, order) };
}
