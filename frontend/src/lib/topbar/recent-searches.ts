/**
 * The layout topbar's recent searches: what is kept in local storage,
 * how it is read back, and how a new search joins it. Pure — the
 * component reads and writes the storage. Split from `./dropdown`
 * so each file stays under the CRAP gate's high-risk bar.
 */

/** localStorage key for the last-N user submissions. Public so the
 *  component imports it instead of redeclaring the literal. */
export const RECENT_STORAGE_KEY = 'ani-gui:recent-searches';

/** Default cap for how many recent submissions we surface. */
export const RECENT_LIMIT = 5;

/** The search a recent entry stands for: its text trimmed, lowercased
 *  and with its spacing collapsed. The backend's search key ignores
 *  case and spacing too (`normalize_query`), so entries with the same
 *  key get the same results. (It also ignores exclusion dashes, which
 *  this key keeps apart.) */
function searchKey(q: string): string {
	return q.trim().toLowerCase().split(/\s+/).join(' ');
}

/** Prepend `query` to `existing`, deduplicate (keeping the freshest
 *  occurrence first), cap at `max`. Pure — caller is responsible for
 *  persisting the result.
 *
 *  Two entries are the same search when they differ only in case or
 *  spacing (`searchKey`), so a query replaces any such entry. Repeats
 *  already present in `existing` are collapsed too; the earliest
 *  occurrence wins. */
export function mergeRecents(
	existing: readonly string[],
	query: string,
	max: number = RECENT_LIMIT
): string[] {
	const seen = new Set<string>([searchKey(query)]);
	const out: string[] = [query];
	for (const x of existing) {
		const key = searchKey(x);
		if (seen.has(key)) continue;
		seen.add(key);
		out.push(x);
	}
	return out.slice(0, max);
}

/** Defensive parse of a localStorage payload that should be a JSON
 *  array of strings. Anything else (null, throwing JSON, wrong
 *  shape) collapses to `[]`. Within an array, non-strings and blank
 *  entries are dropped, the rest trimmed, and a search listed more
 *  than once (`searchKey`) keeps its first entry, before the cap. */
export function parseStoredRecents(raw: string | null, max: number = RECENT_LIMIT): string[] {
	if (raw === null) return [];
	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		return [];
	}
	if (!Array.isArray(parsed)) return [];
	const seen = new Set<string>();
	const out: string[] = [];
	for (const x of parsed) {
		if (typeof x !== 'string') continue;
		const q = x.trim();
		const key = searchKey(q);
		if (q.length === 0 || seen.has(key)) continue;
		seen.add(key);
		out.push(q);
	}
	return out.slice(0, max);
}
