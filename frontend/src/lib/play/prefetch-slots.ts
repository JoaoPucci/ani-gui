/**
 * The prefetch concurrency cap the play cache schedules its fires
 * under ($lib/play/play-cache): a counted set of slots and a queue of
 * the fires waiting for one, kept apart from the cache so each file
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
/** Bumped by the test reset: a slot taken before it neither gives its
 *  count back nor hands its place on after, so the fires a reset
 *  aborts cannot unbalance the next test's cap as they settle. */
let generation = 0;

/**
 * Schedule `fn` under the prefetch concurrency cap. Returns the
 * promise + a `startNow` escape hatch that callers can invoke to cut
 * the queue (used by clicks, and by `getOrFireNow` for a warm a click
 * waits on). `startNow` is a no-op once the slot is already running.
 *
 * Bypassing the cap briefly exceeds PREFETCH_CONCURRENCY by one for
 * each caller that cuts the queue — that's the explicit trade. The cap
 * is for *background* warming; a click, or a warm a click waits on,
 * shouldn't be punished for the warming having queued ahead of it.
 */
export function withSlot<T>(
	fn: () => Promise<T>,
	signal: AbortSignal
): {
	promise: Promise<T>;
	startNow: () => void;
	hasStarted: () => boolean;
} {
	let slotResolver: (() => void) | null = null;
	let started = false;
	const promise = (async () => {
		if (activeFires >= PREFETCH_CONCURRENCY) {
			await new Promise<void>((resolve, reject) => {
				// Aborted while it waits — its page is gone. It leaves
				// the queue without taking a slot, so the fires behind
				// it move up, and it never runs: a fire is a resolve
				// request to the backend. An abort that lands after the
				// slot was granted, before the fire starts, finds the
				// listener gone; `fn` then runs with the signal aborted,
				// which the play cache refuses before it fires (and
				// playStream refuses too), and the slot passes on as it
				// does for any fire that ends.
				const onAbort = () => {
					const idx = fireQueue.indexOf(grant);
					if (idx >= 0) fireQueue.splice(idx, 1);
					slotResolver = null;
					reject(new Error('play-cache: aborted before its turn'));
				};
				const grant = () => {
					signal.removeEventListener('abort', onAbort);
					resolve();
				};
				slotResolver = grant;
				fireQueue.push(grant);
				signal.addEventListener('abort', onAbort, { once: true });
			});
		}
		started = true;
		slotResolver = null;
		activeFires += 1;
		const taken = generation;
		try {
			return await fn();
		} finally {
			if (taken === generation) {
				activeFires -= 1;
				const next = fireQueue.shift();
				if (next) next();
			}
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

/** Test seam: forget every running and queued slot, for
 *  `__resetPlayCacheForTests`, which aborts every entry first. */
export function resetPrefetchSlotsForTests(): void {
	activeFires = 0;
	fireQueue.length = 0;
	generation += 1;
}
