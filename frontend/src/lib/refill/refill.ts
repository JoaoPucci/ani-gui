/**
 * Fetch a read once more when the fill it was served without lands.
 *
 * The backend paces AniList and holds a background fill back rather
 * than let AniList refuse it: a show's episode thumbnails, its banner.
 * A held-back fill is retried once the pause ends, and the read served
 * without it carries the retry's ticket in the `x-ani-gui-refill`
 * header. Without this module the page kept what it was served until
 * the user left and came back; with it, the page waits on the ticket
 * and fetches the read once more when the retry lands.
 *
 * A page that closes lets its tickets go: the retry still goes, since
 * the answer is cached for the next view, but after every fill a page
 * on screen waits for.
 *
 * One {@link RefillWaiter} serves the whole renderer: it keeps a single
 * wait request open for every ticket any page is waiting on, so waiting
 * pages never take more than one of the six connections Chromium opens
 * to the backend.
 */

import { RefillWaiter } from './refill-waiter';

export {
	FAILURE_BACKOFF_MS,
	MAX_FAILURES,
	RefillWaiter,
	type RefillEnded,
	type Sleep,
	type WaitRefills
} from './refill-waiter';

/** The header a read carries its pending refills' tickets in. */
export const REFILL_HEADER = 'x-ani-gui-refill';

/** A read as served, with the tickets of the refills it waits on. */
export interface Served<T> {
	data: T;
	refills: number[];
}

/** The tickets a header names; none for a missing or unreadable one. */
export function parseRefills(header: string | null | undefined): number[] {
	if (!header) return [];
	return header
		.split(',')
		.map((t) => t.trim())
		.filter((t) => /^\d+$/.test(t))
		.map(Number)
		.filter((t) => Number.isSafeInteger(t) && t > 0);
}

/**
 * When `served` waits on refills, wait for them and — once one landed
 * and `signal` has not aborted — fetch once more and hand the result
 * to `apply`. Never fetches more than once, and a refetch that is
 * itself served without a fill is applied as it is.
 */
export async function refetchOnRefill<T>(
	served: Served<T>,
	opts: {
		waiter: RefillWaiter;
		refetch: () => Promise<T>;
		apply: (data: T) => void;
		signal?: AbortSignal;
	}
): Promise<void> {
	if (served.refills.length === 0) return;
	const landed = await opts.waiter.landed(served.refills, opts.signal);
	if (!landed || opts.signal?.aborted) return;
	let data: T;
	try {
		data = await opts.refetch();
	} catch {
		// The page keeps what it has.
		return;
	}
	if (opts.signal?.aborted) return;
	opts.apply(data);
}

/** Fetch a read once more for a page whose refill landed. Each page
 *  fetches on its own for now. */
export function shareRefetch<T>(key: string, start: () => Promise<T>): Promise<T> {
	void key;
	return start();
}
