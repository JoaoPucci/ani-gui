/**
 * What a watch-order read through `latestRead` does when a history link
 * answers ($lib/history/link-deps `onHistoryLinked`): a link can move a
 * row onto an entry of the franchise, so the page is read again. With
 * a view shown, it stays shown until the new read answers, and stays
 * if that read fails (`refresh`). With none yet, the read still out may
 * have read the history before the link, so the show is read afresh. A
 * page that answered with no view (its franchise could not be read)
 * stays as it is: a link does not make a franchise readable. The
 * player's Up next card and the franchise page read through it.
 */
import type { LatestRead } from './latest-read';
import { rereadAnswer, type WatchOrderAnswer } from './loader';

export function followLink<T>(
	reads: LatestRead<WatchOrderAnswer<T> | null>,
	shown: WatchOrderAnswer<T> | null,
	read: (signal: AbortSignal) => Promise<WatchOrderAnswer<T> | null>
): void {
	if (shown === null) {
		reads.start(read);
		return;
	}
	if (shown.view === null) return;
	reads.refresh(async (signal) => {
		return rereadAnswer(await read(signal)) ?? shown;
	});
}
