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
 *
 * The count and its provenance resolve as ONE value. Pinning the
 * number while letting the flag come from whichever answer landed
 * last produces a cap that is exact and marked approximate, or the
 * reverse — and the next click then acts on the half that is wrong.
 */

/** A playable cap and whether the answer it came from was exact. */
export interface Cap {
	count: number | null;
	approximate: boolean;
}

export function createCapAuthority(): {
	recordClickCap: (entryId: string, count: number, approximate: boolean) => void;
	resolveLoaderCap: (entryId: string, cap: Cap) => Cap;
} {
	const pinned = new Map<string, Cap>();
	return {
		recordClickCap: (entryId, count, approximate) => {
			pinned.set(entryId, { count, approximate });
		},
		resolveLoaderCap: (entryId, cap) => {
			const pin = pinned.get(entryId);
			if (!pin) return cap;
			// An exact answer beats an approximate one whichever side it
			// arrived from; "exact" with no count is not an answer at
			// all. Between two of equal confidence the click is fresher.
			if (pin.approximate && !cap.approximate && cap.count != null) return cap;
			return pin;
		}
	};
}
