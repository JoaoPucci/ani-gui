/**
 * The order in which this session wrote what a history removal
 * forgets later: kept positions (watch-position.ts) and a recovery's
 * pending point (resume-after-recovery.ts). A removal notes the moment
 * its rows are gone, and its cleanup — which can wait on Kitsu — then
 * forgets only what was written before that moment, never a point the
 * user made since. A renderer reload ends the cleanup with the session,
 * so the order lives in memory: whatever an earlier session wrote came
 * before.
 */
let last = 0;

/** The next moment in the session's write order, later than every
 *  moment it handed out before. */
export function nextWrite(): number {
	last += 1;
	return last;
}

/** When this session last wrote each key, by the store written to.
 *  A key an earlier session wrote has none. */
const written = new WeakMap<object, Map<string, number>>();

/** Notes a write of `key` to `store`, now. */
export function noteWrite(store: object | null, key: string): void {
	if (!store) return;
	const moments = written.get(store) ?? new Map<string, number>();
	moments.set(key, nextWrite());
	written.set(store, moments);
}

/** Whether this session wrote `key` to `store` after `since` — what a
 *  removal's cleanup leaves. Without `since`, nothing was. */
export function writtenSince(store: object | null, key: string, since?: number): boolean {
	const at = store ? written.get(store)?.get(key) : undefined;
	return (at ?? 0) > (since ?? Infinity);
}
