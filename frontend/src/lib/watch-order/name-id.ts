/**
 * The run's first entry, the one the franchise is named after and its
 * positions are counted from: among the members no prequel link
 * reaches, the earliest start, then the lowest id; where every member
 * has a prequel (the run is one loop), the earliest member.
 */
import { byStart } from './by-start';
import type { RunLink, WatchEntry } from './types';

/** The first entry's id, or null for an empty run. */
export function nameId(entries: readonly WatchEntry[], links: readonly RunLink[]): string | null {
	const reached = new Set(links.filter(([a, b]) => a !== b).map(([, b]) => b));
	const roots = entries.filter((e) => !reached.has(e.id));
	const pool = roots.length > 0 ? roots : entries;
	return [...pool].sort(byStart)[0]?.id ?? null;
}
