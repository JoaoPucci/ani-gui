/**
 * The pages' watch-order reads: the backend's answer for one page
 * (`POST /api/watch-order/:id/<page>`), which decides everything the
 * page draws from the watch order (`$lib/api` `watchOrderShow` and its
 * siblings). The read is not load-bearing for the page: a franchise
 * that cannot be read means no strip, no card, no franchise page (no
 * error, no partial run). History the backend cannot read is its own
 * to handle: it answers a strip without progress.
 *
 * The read belongs to one visit of one page, and `signal` ends it when
 * it ends. The backend holds the response open while it walks the
 * franchise, several rounds of Kitsu reads when Kitsu is slow, on the
 * franchise lane (`$lib/backend/lanes`). Kept after the viewer left, a
 * few such reads would hold all of that lane's connections and queue
 * the next page's read behind them. A visit that ended answers nothing
 * (null), as distinct from a read that failed, which answers that
 * there is no view.
 */
import type { FranchiseView, ShowView, UpNextView } from '$lib/api';

/** What one read answered: the page's view, or null when it could not be read. */
export interface WatchOrderAnswer<T> {
	readonly view: T | null;
}

/** Where the reads come from: `./sources` on the pages, stubs in tests. */
export interface WatchOrderSources {
	readonly show: (id: string, signal?: AbortSignal) => Promise<ShowView>;
	readonly franchise: (id: string, signal?: AbortSignal) => Promise<FranchiseView>;
	readonly upNext: (id: string, episode: number, signal?: AbortSignal) => Promise<UpNextView>;
}

/**
 * `read`'s answer until `signal` ends the visit, or null for a visit
 * that has ended, before the read or while it was out. A read that
 * fails answers no view. Never rejects.
 */
export async function visitRead<T>(
	read: (signal?: AbortSignal) => Promise<T>,
	signal?: AbortSignal
): Promise<WatchOrderAnswer<T> | null> {
	if (signal?.aborted) return null;
	const view = await read(signal).catch(() => null);
	return signal?.aborted ? null : { view };
}

/**
 * A read again's answer, or null to keep what is drawn: the read failed
 * or found no franchise, or it could not read the history (`history`
 * false), so its answer has no progress for that reason and drawing it
 * would take away the progress the page drew.
 */
export function rereadAnswer<T>(fresh: WatchOrderAnswer<T> | null): WatchOrderAnswer<T> | null {
	const view: unknown = fresh?.view ?? null;
	if (fresh === null || view === null) return null;
	const unread = typeof view === 'object' && 'history' in view && view.history === false;
	return unread ? null : fresh;
}
