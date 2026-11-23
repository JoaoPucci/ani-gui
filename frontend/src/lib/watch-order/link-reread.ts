/**
 * What a watch-order read through `latestRead` does when a history link
 * answers ($lib/history/link-deps `onHistoryLinked`): a link can move a
 * row onto an entry of the franchise, so the rows are read again. With
 * an answer shown, they are read against its franchise, which stays
 * shown meanwhile (`refresh`, `rereadRows`). With none yet, the walk
 * still out may have read its rows before the link, so the show is
 * read afresh. The player's Up next card and the franchise page read
 * through it.
 */
import type { LatestRead } from './latest-read';
import { loadWatchOrder, rereadRows, type WatchOrderData, type WatchOrderSources } from './loader';

export function followLink(
	reads: LatestRead<WatchOrderData>,
	showId: string,
	shown: WatchOrderData | null,
	sources: WatchOrderSources
): void {
	if (!showId) return;
	if (shown) reads.refresh((signal) => rereadRows(shown, sources, signal));
	else reads.start((signal) => loadWatchOrder(showId, sources, signal));
}
