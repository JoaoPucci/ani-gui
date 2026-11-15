// The timers a page arms while a case acts on it, each followed to its
// end: it fired, or the page cleared it.
//
// A page that waits on a timer of its own before it acts, such as the
// topbar's live search waiting for the typing to pause, cannot be
// checked for "it did not act" after a fixed sleep: on a slow run the
// timer may not have come due yet, and the negative passes without
// having checked anything. Recording the timers the action armed gives
// a witness instead. Once every one of them has fired or been cleared,
// whatever the page would have done on them has started, and what it
// sends is recorded as it is asked (`watchPageFetches`).
//
// Only timers armed during the call handed to `armedBy` are followed,
// and only through the `setTimeout` and `clearTimeout` globals. A timer
// is matched by the handle `setTimeout` returned: one cleared by any
// other value stays armed here, and `done` times out rather than
// passing.

import { onTestFinished, vi } from 'vitest';

export interface ArmedTimers {
	/** How many of the timers armed have neither fired nor been cleared. */
	readonly pending: number;
	/** Resolves once every timer armed has fired or been cleared. */
	done(what: string, timeoutMs?: number): Promise<void>;
}

/**
 * Run `act` and follow every timer it arms through `setTimeout` to its
 * end. `act` must arm them before it returns, as an event handler
 * dispatched by the case does.
 */
export function armedBy(act: () => void): ArmedTimers {
	const open = new Set<unknown>();
	const realSet = globalThis.setTimeout;
	const realClear = globalThis.clearTimeout;
	vi.stubGlobal('clearTimeout', (handle?: Parameters<typeof clearTimeout>[0]) => {
		open.delete(handle);
		realClear(handle);
	});
	onTestFinished(() => {
		vi.unstubAllGlobals();
	});
	const following = ((run: (...args: unknown[]) => void, ms?: number, ...args: unknown[]) => {
		const handle = realSet(() => {
			open.delete(handle);
			run(...args);
		}, ms);
		open.add(handle);
		return handle;
	}) as unknown as typeof setTimeout;
	globalThis.setTimeout = following;
	try {
		act();
	} finally {
		globalThis.setTimeout = realSet;
	}
	return {
		get pending() {
			return open.size;
		},
		async done(what, timeoutMs = 8000) {
			const deadline = Date.now() + timeoutMs;
			while (open.size > 0) {
				if (Date.now() > deadline) {
					throw new Error(`timed out waiting for ${what}: ${open.size} timer(s) still armed`);
				}
				await new Promise((r) => realSet(r, 10));
			}
		}
	};
}
