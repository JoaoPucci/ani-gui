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

/** When this session last noted each key, by the store it lives in.
 *  A key an earlier session wrote has none. */
export class Moments {
	private readonly at = new WeakMap<object, Map<string, number>>();

	/** Notes `key` in `store`, now. */
	note(store: object | null, key: string): void {
		if (!store) return;
		const moments = this.at.get(store) ?? new Map<string, number>();
		moments.set(key, nextWrite());
		this.at.set(store, moments);
	}

	/** Whether `key` in `store` was noted after `since`. Without
	 *  `since`, it was not. */
	since(store: object | null, key: string, since?: number): boolean {
		const at = store ? this.at.get(store)?.get(key) : undefined;
		return (at ?? 0) > (since ?? Infinity);
	}
}

/** The session's writes of each key: what a removal's cleanup leaves. */
export const writes = new Moments();
/** The session's picks of a key's episode that kept the point there:
 *  a cleanup that forgets the point restarts it instead. */
export const picks = new Moments();
