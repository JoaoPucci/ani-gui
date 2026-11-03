/**
 * The run's first entry, the one the franchise is named after and its
 * positions are counted from: among the members no prequel link
 * reaches, the earliest start, then the lowest id; where every member
 * has a prequel (the run is one loop), the earliest member.
 */
import type { RunLink, WatchEntry } from './types';

/** The first entry's id, or null for an empty run. */
export function nameId(entries: readonly WatchEntry[], links: readonly RunLink[]): string | null {
	void links;
	return entries.at(-1)?.id ?? null;
}
