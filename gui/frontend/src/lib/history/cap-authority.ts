/**
 * Which playable cap wins for a Continue Watching row.
 *
 * Two sources write a row's cap and they can land out of order. The
 * loader's background availability probe is paced by the scraper
 * gate, so a click on a cold-cache card often resolves its own
 * INTERACTIVE lookup first; the queued background probe then lands
 * afterwards and would otherwise overwrite the exact click-time
 * answer. That matters because a breaker-refused background detail
 * fetch is allowed to fall back to an approximate episode count
 * (the +1 half-episode case), and the next click reads a numeric cap
 * as authoritative — skipping its own lookup and potentially
 * selecting an episode that doesn't exist.
 *
 * So a cap learned at click time pins the row: later loader
 * callbacks for that entry carry the pinned value instead of their
 * own. A newer click supersedes an older pin (the user's latest
 * interactive answer is always the freshest), and rows nobody
 * clicked pass through untouched.
 */

export function createCapAuthority(): {
	recordClickCap: (entryId: string, count: number) => void;
	resolveLoaderCount: (entryId: string, count: number | null) => number | null;
} {
	const pinned = new Map<string, number>();
	return {
		recordClickCap: (entryId, count) => {
			pinned.set(entryId, count);
		},
		resolveLoaderCount: (entryId, count) => pinned.get(entryId) ?? count
	};
}
