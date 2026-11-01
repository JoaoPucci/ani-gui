/**
 * Fetch a read once more when the fill it was served without lands.
 *
 * Nothing is retried yet: no read carries a ticket, a waiter reports
 * nothing landed, and no read is fetched again.
 */

/** The header a read carries its pending refills' tickets in. */
export const REFILL_HEADER = 'x-ani-gui-refill';

/** A read as served, with the tickets of the refills it waits on. */
export interface Served<T> {
	data: T;
	refills: number[];
}

/** One ticket's end, as the wait route reports it. */
export interface RefillEnded {
	ticket: number;
	landed: boolean;
}

/** Ask the backend which of `tickets` have ended. */
export type WaitRefills = (tickets: number[], signal: AbortSignal) => Promise<RefillEnded[]>;

/** How a waiter sleeps between failed waits; injectable for tests. */
export type Sleep = (ms: number) => Promise<void>;

/** Failed waits in a row before the waiter gives every page up. */
export const MAX_FAILURES = 3;
/** The pause after a failed wait. */
export const FAILURE_BACKOFF_MS = 2000;

/** The tickets a header names: none yet. */
export function parseRefills(header: string | null | undefined): number[] {
	void header;
	return [];
}

/** Every ticket the renderer waits on. */
export class RefillWaiter {
	constructor(wait: WaitRefills, sleep?: Sleep, release?: (tickets: number[]) => void) {
		void wait;
		void sleep;
		void release;
	}

	/** Nothing lands yet. */
	landed(tickets: number[], signal?: AbortSignal): Promise<boolean> {
		void tickets;
		void signal;
		return Promise.resolve(false);
	}
}

/** Fetch once more after a refill landed: none does yet. */
export async function refetchOnRefill<T>(
	served: Served<T>,
	opts: {
		waiter: RefillWaiter;
		refetch: () => Promise<T>;
		apply: (data: T) => void;
		signal?: AbortSignal;
	}
): Promise<void> {
	void served;
	void opts;
}
