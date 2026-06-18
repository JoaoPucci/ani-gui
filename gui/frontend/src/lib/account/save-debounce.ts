// Per-key coalescing debouncer for the list editor's tracker writes. Mashing
// Save on the same show would otherwise fire a full multi-tracker fan-out per
// click and trip the trackers' rate limit (AniList 429). Coalescing collapses
// a burst on one show (keyed by Kitsu id) into a single write of the latest
// payload — the UI still updates optimistically on every click, only the
// network call is debounced. Distinct shows, and saves spaced beyond the
// window, run independently. setTimeout is injected so the timing is unit-
// tested with fake timers.

type Handle = ReturnType<typeof setTimeout>;

export interface Debouncer {
	/** Schedule `run` for `key`, replacing (cancelling) any pending run for the
	 *  same key so only the latest fires. */
	schedule(key: string, run: () => void): void;
	/** Drop any pending run for `key` without running it (e.g. a Remove
	 *  superseding a queued Save). */
	cancel(key: string): void;
	/** Whether a run is currently pending for `key`. */
	pending(key: string): boolean;
}

export function createDebouncer(
	delayMs: number,
	// Arrow wrappers, not bare `setTimeout`/`clearTimeout` references: calling
	// them as methods of this object (`timers.set(...)`) invokes them with the
	// wrong receiver, which browsers reject with "Illegal invocation". The
	// wrappers call them as free globals.
	timers: { set: (cb: () => void, ms: number) => Handle; clear: (h: Handle) => void } = {
		set: (cb, ms) => setTimeout(cb, ms),
		clear: (h) => clearTimeout(h)
	}
): Debouncer {
	const handles = new Map<string, Handle>();
	return {
		schedule(key, run) {
			const existing = handles.get(key);
			if (existing !== undefined) timers.clear(existing);
			handles.set(
				key,
				timers.set(() => {
					handles.delete(key);
					run();
				}, delayMs)
			);
		},
		cancel(key) {
			const h = handles.get(key);
			if (h !== undefined) {
				timers.clear(h);
				handles.delete(key);
			}
		},
		pending(key) {
			return handles.has(key);
		}
	};
}
