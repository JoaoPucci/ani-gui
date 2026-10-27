/**
 * Which rows this session has handed the backend's warm.
 *
 * Every list view filters through `filterAvailable`, which hands the
 * warm the ids the cache does not know yet. Home mounts again on
 * every return to it, so without a record of what was already handed
 * in, the same ids went to the warm on every visit — while their walk
 * was still out, or moments after it ended without a verdict. The
 * backend holds a row once in its queue and walks it once at a time
 * however often it is handed in, so this is not what keeps walks from
 * piling up; it is what keeps a Home visit from asking for work that
 * is already asked for.
 *
 * An id is handed in once. An id the cache still does not know when
 * a list shows it again — its walk ended without a verdict, or never
 * got to it — is handed in again once {@link WARM_RETRY_MS} has passed
 * since it last was. A warm that never reached the backend is
 * forgotten, so the next list hands it in at once.
 *
 * Keys are `mode:id`: the provider catalogues sub and dub separately,
 * and each is a row, and a warm, of its own.
 */

/** How long after a warm an id still uncached is handed in again.
 *  Two minutes: past the minute a single walk may take, and short
 *  enough that a show whose walk found nothing to say is asked about
 *  again within the same sitting. */
export const WARM_RETRY_MS = 120_000;

/** Whether an id last warmed at `sentAt` (never, if undefined) is due
 *  a warm at `now`. */
export function dueForWarm(sentAt: number | undefined, now: number, retryMs: number): boolean {
	return sentAt === undefined || now - sentAt >= retryMs;
}

export interface WarmLedger {
	/** The keys among `keys` due a warm at `now`, now marked warmed. */
	take(keys: string[], now: number): string[];
	/** The warm of `keys` never reached the backend: due again. */
	forget(keys: string[]): void;
}

export function createWarmLedger(retryMs: number = WARM_RETRY_MS): WarmLedger {
	const sent = new Map<string, number>();
	return {
		take(keys, now) {
			const due = keys.filter((key) => dueForWarm(sent.get(key), now, retryMs));
			for (const key of due) sent.set(key, now);
			return due;
		},
		forget(keys) {
			for (const key of keys) sent.delete(key);
		}
	};
}

/** The renderer's one ledger: it lives as long as the session. */
export const warmLedger = createWarmLedger();
