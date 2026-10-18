/**
 * The prefetch concurrency cap the play cache schedules its fires
 * under ($lib/play/play-cache), kept apart from the cache so each file
 * stays inside the complexity bar.
 */

/**
 * Cap on the number of `fire` calls running at once. Twelve concurrent
 * resolves from a single page mount overloads the backend (CPU
 * contention plus the provider's rate limit) and slows the user's own
 * click; queueing past the cap keeps the active set small while still
 * eventually warming every visible episode.
 *
 * Tunable, but the backend paces provider traffic again through
 * ScraperGate, so raising this mostly moves the queue rather than the
 * throughput. Lower it if the ratio of prefetched-but-unused entries
 * becomes wasteful.
 */
const PREFETCH_CONCURRENCY = 2;
let activeFires = 0;
const fireQueue: Array<() => void> = [];

/**
 * Schedule `fn` under the prefetch concurrency cap. Returns the
 * promise + a `startNow` escape hatch that callers can invoke to cut
 * the queue (used by clicks). `startNow` is a no-op once the slot is
 * already running.
 *
 * Bypassing the cap briefly exceeds PREFETCH_CONCURRENCY by one for
 * each click — that's the explicit trade. The cap is for *background*
 * warming; foreground clicks shouldn't be punished for the warming
 * having queued ahead of them.
 */
export function withSlot<T>(fn: () => Promise<T>): {
	promise: Promise<T>;
	startNow: () => void;
	hasStarted: () => boolean;
} {
	let slotResolver: (() => void) | null = null;
	let started = false;
	const promise = (async () => {
		if (activeFires >= PREFETCH_CONCURRENCY) {
			await new Promise<void>((resolve) => {
				slotResolver = resolve;
				fireQueue.push(resolve);
			});
		}
		started = true;
		slotResolver = null;
		activeFires += 1;
		try {
			return await fn();
		} finally {
			activeFires -= 1;
			const next = fireQueue.shift();
			if (next) next();
		}
	})();
	const startNow = () => {
		if (started || !slotResolver) return;
		// Pull the resolver out of the queue, then resolve it. Bypasses
		// the cap; activeFires temporarily exceeds PREFETCH_CONCURRENCY.
		const idx = fireQueue.indexOf(slotResolver);
		if (idx >= 0) fireQueue.splice(idx, 1);
		const r = slotResolver;
		slotResolver = null;
		r();
	};
	return { promise, startNow, hasStarted: () => started };
}

/** Test seam: forget every running and queued slot. */
export function resetPrefetchSlotsForTests(): void {
	activeFires = 0;
	fireQueue.length = 0;
}
