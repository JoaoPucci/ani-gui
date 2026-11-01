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

/** Ask the backend which of `tickets` have ended; resolves with the
 *  ended ones, or `[]` when none did before the backend's own limit. */
export type WaitRefills = (tickets: number[], signal: AbortSignal) => Promise<RefillEnded[]>;

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

interface Listener {
	tickets: Set<number>;
	done: (landed: boolean) => void;
}

/** How a waiter sleeps between failed waits; injectable for tests. */
export type Sleep = (ms: number) => Promise<void>;

const realSleep: Sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** Failed waits in a row before the waiter gives every page up. */
export const MAX_FAILURES = 3;
/** The pause after a failed wait. */
export const FAILURE_BACKOFF_MS = 2000;

/** Every ticket the renderer waits on, behind one wait request. */
export class RefillWaiter {
	private listeners = new Set<Listener>();
	private inFlight: AbortController | null = null;
	/** The tickets the open wait names. */
	private asked = new Set<number>();
	private running = false;

	constructor(
		private readonly wait: WaitRefills,
		private readonly sleep: Sleep = realSleep,
		private readonly release: (tickets: number[]) => void = () => {}
	) {}

	/**
	 * Resolves `true` once one of `tickets` landed, `false` once all
	 * of them ended unlanded, the waiter gave up, or `signal` aborted.
	 */
	landed(tickets: number[], signal?: AbortSignal): Promise<boolean> {
		if (tickets.length === 0 || signal?.aborted) return Promise.resolve(false);
		return new Promise<boolean>((resolve) => {
			const listener: Listener = {
				tickets: new Set(tickets),
				done: (landed) => {
					this.listeners.delete(listener);
					resolve(landed);
				}
			};
			this.listeners.add(listener);
			signal?.addEventListener(
				'abort',
				() => {
					if (!this.listeners.has(listener)) return;
					listener.done(false);
					// No page shows these any more: their retries rank
					// after every fill still on screen — once the open
					// wait, which still names them, is replaced by one
					// that does not.
					const still = new Set(this.pending());
					const gone = [...listener.tickets].filter((t) => !still.has(t));
					if (gone.length === 0) return;
					this.release(gone);
					this.inFlight?.abort();
				},
				{ once: true }
			);
			// A wait already open that does not name these tickets
			// starts over with them in.
			if (this.inFlight && tickets.some((t) => !this.asked.has(t))) this.inFlight.abort();
			void this.run();
		});
	}

	private pending(): number[] {
		const all = new Set<number>();
		for (const l of this.listeners) for (const t of l.tickets) all.add(t);
		return [...all];
	}

	private settle(ended: RefillEnded[]): void {
		for (const { ticket, landed } of ended) {
			for (const l of [...this.listeners]) {
				if (!l.tickets.has(ticket)) continue;
				if (landed) {
					l.done(true);
				} else {
					l.tickets.delete(ticket);
					if (l.tickets.size === 0) l.done(false);
				}
			}
		}
	}

	private async run(): Promise<void> {
		if (this.running) return;
		this.running = true;
		let failures = 0;
		try {
			for (let tickets = this.pending(); tickets.length > 0; tickets = this.pending()) {
				const controller = new AbortController();
				this.inFlight = controller;
				this.asked = new Set(tickets);
				try {
					this.settle(await this.wait(tickets, controller.signal));
					failures = 0;
				} catch {
					// Restarted for new tickets: not a failure.
					if (controller.signal.aborted) continue;
					failures += 1;
					if (failures >= MAX_FAILURES) {
						for (const l of [...this.listeners]) l.done(false);
						break;
					}
					await this.sleep(FAILURE_BACKOFF_MS);
				} finally {
					if (this.inFlight === controller) {
						this.inFlight = null;
						this.asked = new Set();
					}
				}
			}
		} finally {
			this.running = false;
		}
	}
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
