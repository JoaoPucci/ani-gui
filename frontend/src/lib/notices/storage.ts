/** What the center keeps across restarts: events only, each read back
 *  field by field. */
import { lifetimeOf, type Notice } from './kinds';
import { prune } from './retention';
import { storedRow } from './stored-row';

export const STORAGE_KEY = 'ani-gui.notices.v1';

/** What survives a restart: events only. */
export function serialize(list: readonly Notice[]): string {
	return JSON.stringify({
		v: 1,
		rows: list.filter((n) => lifetimeOf(n.kind) === 'event')
	});
}

/** The stored rows, each checked field by field; anything that is not
 *  a stored event is dropped, then retention applies. */
export function restore(raw: string | null, now: number): Notice[] {
	if (!raw) return [];
	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		return [];
	}
	if (typeof parsed !== 'object' || parsed === null) return [];
	const { v, rows } = parsed as { v?: unknown; rows?: unknown };
	if (v !== 1 || !Array.isArray(rows)) return [];
	const seen = new Set<string>();
	const kept: Notice[] = [];
	for (const r of rows) {
		const n = storedRow(r);
		if (n && !seen.has(n.key)) {
			seen.add(n.key);
			kept.push(n);
		}
	}
	return prune(kept, now);
}
