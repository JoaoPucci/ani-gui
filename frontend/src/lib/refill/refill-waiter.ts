/**
 * The renderer's one waiter on refills (`./refill`): every ticket any
 * page waits on, behind a single wait request, so waiting pages never
 * take more than one of the six connections Chromium opens to the
 * backend. Split from `./refill` so each file stays inside the CRAP
 * gate's per-file bar.
 */
import { settle, type Listener } from './refill-settle';

/** One ticket's end, as the wait route reports it. */
export interface RefillEnded {
	ticket: number;
	landed: boolean;
}

/** Ask the backend which of `tickets` have ended; resolves with the
 *  ended ones, or `[]` when none did before the backend's own limit. */
export type WaitRefills = (tickets: number[], signal: AbortSignal) => Promise<RefillEnded[]>;

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
	 * A page that closed before its read came back lets its tickets go
	 * at once.
	 */
	landed(tickets: number[], signal?: AbortSignal): Promise<boolean> {
		if (signal?.aborted) this.letGo(tickets);
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
					if (this.letGo([...listener.tickets])) this.inFlight?.abort();
				},
				{ once: true }
			);
			// A wait already open that does not name these tickets
			// starts over with them in.
			if (this.inFlight && tickets.some((t) => !this.asked.has(t))) this.inFlight.abort();
			void this.run();
		});
	}

	/**
	 * Let go of the tickets among `tickets` no page waits on: their
	 * retries rank after every fill still on screen. `true` when any
	 * was let go.
	 */
	letGo(tickets: number[]): boolean {
		const still = new Set(this.pending());
		const gone = [...new Set(tickets)].filter((t) => !still.has(t));
		if (gone.length === 0) return false;
		this.release(gone);
		return true;
	}

	private pending(): number[] {
		const all = new Set<number>();
		for (const l of this.listeners) for (const t of l.tickets) all.add(t);
		return [...all];
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
					settle(this.listeners, await this.wait(tickets, controller.signal));
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
