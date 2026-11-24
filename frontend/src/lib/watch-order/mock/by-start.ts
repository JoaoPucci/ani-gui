/**
 * Start-date order for run members, and the order setting's two values.
 * Part of the mockups' ordering; `./order` gathers the pieces.
 */
import type { MockEntry } from './types';

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
