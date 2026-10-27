// A tracker write of a show that has settled, told to every view that
// shows that tracker's list. The list editor reconciles its own entry by
// reading it again once its save settles; a view that is not the editor
// — the home page's Watch Later row, a detail page opened while a
// progress write waited — learns of the write here and reads its list
// again the same way. The re-read is what the view shows: nothing here
// says what the tracker now holds.

import type { Provider } from './types';

/** How one tracker's write of a show ended: taken, refused or failed
 *  for good, or dropped unsent — a newer write took every field, or the
 *  account it was made for left. */
export type WriteEnd = 'landed' | 'failed' | 'dropped';

/** Which surface made the write. */
export type WriteSource = 'editor' | 'progress';

/** A write of `kitsuId` that settled on `providers`, the trackers whose
 *  list it may have changed. */
export interface ListChange {
	kitsuId: string;
	providers: Provider[];
	source: WriteSource;
}

/** How a tracker write ended, from how its promise settled. */
export const writeEnd: (settled: PromiseSettledResult<unknown>) => WriteEnd = () => 'failed';

/** The change a write of `kitsuId` made across trackers, from how each
 *  ended; `null` when none of them can have changed a list. */
export const changeOf: (
	kitsuId: string,
	source: WriteSource,
	ends: Partial<Record<Provider, WriteEnd>>
) => ListChange | null = () => null;

/** Whether a view showing the lists of `shown` reads them again for
 *  `change`. */
export const railNeedsReread: (change: ListChange, shown: readonly Provider[]) => boolean = () =>
	false;

/** Whether a detail page showing `kitsuId`'s entry reads it again for
 *  `change`. */
export const entryNeedsReread: (change: ListChange, kitsuId: string | null) => boolean = () =>
	false;

/** The channel the writes announce on and the views listen to. */
export interface ListChanges {
	announce(change: ListChange): void;
	/** Listen for changes; returns what stops listening. */
	subscribe(listener: (change: ListChange) => void): () => void;
}

export function createListChanges(): ListChanges {
	return {
		announce: () => {},
		subscribe: () => () => {}
	};
}

/** The app's one channel. */
export const listChanges = createListChanges();
