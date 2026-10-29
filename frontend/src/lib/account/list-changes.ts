// A tracker write of a show that has settled, told to every view that
// shows that tracker's list. The list editor reconciles its own entry by
// reading it again once its save settles; a view that is not the editor
// — the home page's Watch Later row, a detail page opened while a
// progress write waited — learns of the write here and reads its list
// again the same way. The re-read is what the view shows: nothing here
// says what the tracker now holds.

import type { Provider } from './types';
import { SupersededError } from './save-when-allowed';

/** The trackers in the order a change names them. */
const TRACKERS: readonly Provider[] = ['anilist', 'mal', 'inhouse'];

/** How one tracker's write of a show ended: taken, refused or failed
 *  for good, or dropped unsent — a newer write took every field, or the
 *  account it was made for left. */
export type WriteEnd = 'landed' | 'failed' | 'dropped';

/** Which surface made the write. */
export type WriteSource = 'editor' | 'progress';

/** A write of `kitsuId` that settled on `providers`: the trackers it
 *  was sent to, whose list it may have changed — `landed` those that
 *  took it, the rest refused it or failed after it went out. */
export interface ListChange {
	kitsuId: string;
	providers: Provider[];
	landed: Provider[];
	source: WriteSource;
}

/** How a tracker write ended, from how its promise settled. */
export function writeEnd(settled: PromiseSettledResult<unknown>): WriteEnd {
	if (settled.status === 'fulfilled') return 'landed';
	return settled.reason instanceof SupersededError ? 'dropped' : 'failed';
}

/** The change a write of `kitsuId` made across trackers, from how each
 *  ended; `null` when none of them can have changed a list. */
export function changeOf(
	kitsuId: string,
	source: WriteSource,
	ends: Partial<Record<Provider, WriteEnd>>
): ListChange | null {
	// A dropped write sent nothing; a failed one went out, and a view
	// that reads the tracker live reads it again.
	const providers = TRACKERS.filter((p) => ends[p] === 'landed' || ends[p] === 'failed');
	const landed = TRACKERS.filter((p) => ends[p] === 'landed');
	return providers.length === 0 ? null : { kitsuId, providers, landed, source };
}

/** Whether a view showing the lists of `shown` reads them again for
 *  `change`. */
export function railNeedsReread(change: ListChange, shown: readonly Provider[]): boolean {
	// A mapped progress write can cache completed earlier entries before
	// failing on its target. Its final failure does not mean the cached
	// list stayed unchanged. Editor writes affect one entry per tracker.
	const affected = change.source === 'progress' ? change.providers : change.landed;
	return affected.some((p) => shown.includes(p));
}

/** Whether a detail page showing `kitsuId`'s entry reads it again for
 *  `change`. */
export function entryNeedsReread(change: ListChange, kitsuId: string | null): boolean {
	// The editor's own save is reconciled by the editor that made it, or
	// by the one mounted for the show since (`editor-flights`).
	return change.source === 'progress' && kitsuId !== null && change.kitsuId === kitsuId;
}

/** The channel the writes announce on and the views listen to. */
export interface ListChanges {
	announce(change: ListChange): void;
	/** Listen for changes; returns what stops listening. */
	subscribe(listener: (change: ListChange) => void): () => void;
}

export function createListChanges(): ListChanges {
	const listeners = new Set<(change: ListChange) => void>();
	return {
		announce(change) {
			// A copy, so a listener that stops listening while it is told
			// does not cut the others short.
			for (const listener of [...listeners]) {
				try {
					listener(change);
				} catch {
					// One view going wrong must not keep the others stale.
				}
			}
		},
		subscribe(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		}
	};
}

/** Run one tracker's write, recording in `ends` how it ended, and
 *  return it as it came. */
export function recordEnd<T>(
	ends: Partial<Record<Provider, WriteEnd>>,
	provider: Provider,
	write: Promise<T>
): Promise<T> {
	write.then(
		(value) => (ends[provider] = writeEnd({ status: 'fulfilled', value })),
		(reason: unknown) => (ends[provider] = writeEnd({ status: 'rejected', reason }))
	);
	return write;
}

/** Announce the change a write of `kitsuId` made, if it made one. */
export function announceEnds(
	channel: ListChanges,
	kitsuId: string,
	source: WriteSource,
	ends: Partial<Record<Provider, WriteEnd>>
): void {
	const change = changeOf(kitsuId, source, ends);
	if (change) channel.announce(change);
}

/** The app's one channel. */
export const listChanges = createListChanges();
