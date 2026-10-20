/**
 * The play cache's sweep over one show's entries: the loop
 * `clearForShow` and the background sweeps share, kept apart from the
 * cache so each caller states only which of the show's entries go.
 */

/** What a sweep needs of an entry: a fire to abort. */
export interface SweptEntry {
	abortController: AbortController;
}

/** Abort and forget the entries of `showId` that `drop` names. A
 *  show's keys start with its id and a `|` (`makeKey`). */
export function sweepShow<K extends string, E extends SweptEntry>(
	cache: Map<K, E>,
	showId: string,
	drop: (key: K, entry: E) => boolean
): void {
	const prefix = `${showId}|`;
	for (const [key, entry] of cache) {
		if (!key.startsWith(prefix) || !drop(key, entry)) continue;
		entry.abortController.abort();
		cache.delete(key);
	}
}
