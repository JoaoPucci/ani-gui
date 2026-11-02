import fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';
import {
	FAILURE_BACKOFF_MS,
	MAX_FAILURES,
	RefillWaiter,
	parseRefills,
	refetchOnRefill,
	shareRefetch,
	type RefillEnded,
	type WaitRefills
} from './refill';

/** A wait route the test answers by hand: each call is held until the
 *  test resolves or rejects it. */
function manualWait() {
	const calls: {
		tickets: number[];
		signal: AbortSignal;
		resolve: (e: RefillEnded[]) => void;
		reject: (e: unknown) => void;
	}[] = [];
	const wait: WaitRefills = (tickets, signal) =>
		new Promise<RefillEnded[]>((resolve, reject) => {
			calls.push({ tickets, signal, resolve, reject });
			signal.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')));
		});
	return { wait, calls };
}

const flush = () => new Promise<void>((r) => setTimeout(r, 0));
const noSleep = () => Promise.resolve();

describe('parseRefills', () => {
	it('reads the comma-separated tickets', () => {
		expect(parseRefills('3,4')).toEqual([3, 4]);
		expect(parseRefills(' 7 ')).toEqual([7]);
	});
	it('reads nothing from a missing or garbled header', () => {
		expect(parseRefills(null)).toEqual([]);
		expect(parseRefills(undefined)).toEqual([]);
		expect(parseRefills('')).toEqual([]);
		expect(parseRefills('x,-1,0,1.5')).toEqual([]);
	});
});

describe('RefillWaiter', () => {
	it('resolves true when a ticket landed', async () => {
		const { wait, calls } = manualWait();
		const waiter = new RefillWaiter(wait, noSleep);
		const landed = waiter.landed([3]);
		await flush();
		expect(calls).toHaveLength(1);
		expect(calls[0].tickets).toEqual([3]);
		calls[0].resolve([{ ticket: 3, landed: true }]);
		expect(await landed).toBe(true);
	});

	it('resolves false once every ticket ended unlanded', async () => {
		const { wait, calls } = manualWait();
		const waiter = new RefillWaiter(wait, noSleep);
		const landed = waiter.landed([3, 4]);
		await flush();
		calls[0].resolve([{ ticket: 3, landed: false }]);
		await flush();
		expect(calls).toHaveLength(2);
		expect(calls[1].tickets).toEqual([4]);
		calls[1].resolve([{ ticket: 4, landed: false }]);
		expect(await landed).toBe(false);
	});

	it('asks again after a wait that ended with nothing', async () => {
		const { wait, calls } = manualWait();
		const waiter = new RefillWaiter(wait, noSleep);
		const landed = waiter.landed([5]);
		await flush();
		calls[0].resolve([]);
		await flush();
		expect(calls).toHaveLength(2);
		calls[1].resolve([{ ticket: 5, landed: true }]);
		expect(await landed).toBe(true);
	});

	it('keeps one wait open for every page, restarting it with new tickets', async () => {
		const { wait, calls } = manualWait();
		const waiter = new RefillWaiter(wait, noSleep);
		const a = waiter.landed([1]);
		await flush();
		const b = waiter.landed([2]);
		await flush();
		expect(calls[0].signal.aborted).toBe(true);
		const open = calls.filter((c) => !c.signal.aborted);
		expect(open).toHaveLength(1);
		expect(open[0].tickets.sort()).toEqual([1, 2]);
		open[0].resolve([
			{ ticket: 1, landed: true },
			{ ticket: 2, landed: false }
		]);
		expect(await a).toBe(true);
		expect(await b).toBe(false);
	});

	it('lets a closed page go without a refetch, and stops asking for it', async () => {
		const { wait, calls } = manualWait();
		const waiter = new RefillWaiter(wait, noSleep);
		const page = new AbortController();
		const landed = waiter.landed([9], page.signal);
		await flush();
		page.abort();
		expect(await landed).toBe(false);
		// The open wait named the closed page's ticket: it is dropped,
		// and with nothing left to wait on, none replaces it.
		expect(calls[0].signal.aborted).toBe(true);
		await flush();
		expect(calls).toHaveLength(1);
	});

	it('does not restart a wait that already names every ticket asked for', async () => {
		const { wait, calls } = manualWait();
		const waiter = new RefillWaiter(wait, noSleep);
		void waiter.landed([1, 2]);
		await flush();
		void waiter.landed([2]);
		await flush();
		expect(calls).toHaveLength(1);
		expect(calls[0].signal.aborted).toBe(false);
	});

	it('lets the tickets a closed page alone showed go', async () => {
		const { wait, calls } = manualWait();
		const release = vi.fn();
		const waiter = new RefillWaiter(wait, noSleep, release);
		const left = new AbortController();
		void waiter.landed([1, 2], left.signal);
		void waiter.landed([2, 3]);
		await flush();
		left.abort();
		expect(release).toHaveBeenCalledWith([1]);
		await flush();
		// The wait goes on for the page still open, without ticket 1.
		expect(calls.filter((c) => !c.signal.aborted).map((c) => c.tickets)).toEqual([[2, 3]]);
	});

	it('gives every page up after repeated failures', async () => {
		const { wait, calls } = manualWait();
		const sleep = vi.fn(noSleep);
		const waiter = new RefillWaiter(wait, sleep);
		const landed = waiter.landed([4]);
		for (let i = 0; i < MAX_FAILURES; i++) {
			await flush();
			calls[i].reject(new Error('offline'));
		}
		expect(await landed).toBe(false);
		expect(sleep).toHaveBeenCalledTimes(MAX_FAILURES - 1);
	});

	it('waits a moment between failed waits when no pause is given', async () => {
		vi.useFakeTimers();
		try {
			const { wait, calls } = manualWait();
			const waiter = new RefillWaiter(wait);
			const landed = waiter.landed([1]);
			await vi.advanceTimersByTimeAsync(0);
			calls[0].reject(new Error('offline'));
			await vi.advanceTimersByTimeAsync(FAILURE_BACKOFF_MS - 1);
			expect(calls).toHaveLength(1);
			await vi.advanceTimersByTimeAsync(1);
			expect(calls).toHaveLength(2);
			calls[1].resolve([{ ticket: 1, landed: true }]);
			expect(await landed).toBe(true);
		} finally {
			vi.useRealTimers();
		}
	});

	it('lets go of the tickets of a page that closed before its read came back', async () => {
		const { wait, calls } = manualWait();
		const release = vi.fn();
		const waiter = new RefillWaiter(wait, noSleep, release);
		void waiter.landed([2]);
		await flush();
		const left = new AbortController();
		left.abort();
		expect(await waiter.landed([1, 2], left.signal)).toBe(false);
		// Ticket 2 is still shown by the page that waits on it.
		expect(release).toHaveBeenCalledWith([1]);
		await flush();
		expect(calls.map((c) => c.tickets)).toEqual([[2]]);
	});

	it('lets go of the tickets of a read no page waits on', () => {
		const { wait } = manualWait();
		const release = vi.fn();
		const waiter = new RefillWaiter(wait, noSleep, release);
		void waiter.landed([3]);
		waiter.letGo([3, 4, 4]);
		expect(release).toHaveBeenCalledWith([4]);
		waiter.letGo([3]);
		waiter.letGo([]);
		expect(release).toHaveBeenCalledTimes(1);
	});

	/** Pages register their tickets and close in any order, some before
	 *  their read came back: a release never names a ticket an open page
	 *  waits on, and once every step has run, each ticket a closed page
	 *  held that no open page holds was released after it was last
	 *  handed to the waiter. */
	it('lets go of every ticket no open page holds, whenever its page closed', async () => {
		const ticket = fc.integer({ min: 1, max: 5 });
		const page = fc.record({
			tickets: fc.uniqueArray(ticket, { minLength: 1, maxLength: 3 }),
			closesBeforeRead: fc.boolean()
		});
		await fc.assert(
			fc.asyncProperty(
				fc.array(page, { minLength: 1, maxLength: 6 }),
				fc.array(fc.nat(), { maxLength: 6 }),
				async (pages, closings) => {
					const { wait } = manualWait();
					const log: { step: number; released: number[] }[] = [];
					let step = 0;
					const open = new Map<number, number[]>();
					const heldWhenReleased: number[] = [];
					const release = (tickets: number[]) => {
						const held = new Set([...open.values()].flat());
						heldWhenReleased.push(...tickets.filter((t) => held.has(t)));
						log.push({ step, released: tickets });
					};
					const waiter = new RefillWaiter(wait, noSleep, release);
					const lastHanded = new Map<number, number>();
					const controllers = new Map<number, AbortController>();
					const closed = new Set<number>();
					const close = (i: number) => {
						open.delete(i);
						closed.add(i);
						controllers.get(i)?.abort();
					};
					for (const [i, p] of pages.entries()) {
						step += 1;
						const c = new AbortController();
						controllers.set(i, c);
						if (p.closesBeforeRead) {
							closed.add(i);
							c.abort();
						} else {
							open.set(i, p.tickets);
						}
						for (const t of p.tickets) lastHanded.set(t, step);
						void waiter.landed(p.tickets, c.signal);
						await flush();
						const at = closings[i];
						if (at !== undefined && open.size > 0) {
							step += 1;
							const keys = [...open.keys()];
							close(keys[at % keys.length]);
							await flush();
						}
					}
					expect(heldWhenReleased).toEqual([]);
					const stillHeld = new Set([...open.values()].flat());
					for (const i of closed) {
						for (const t of pages[i].tickets) {
							if (stillHeld.has(t)) continue;
							const releasedAt = log.filter((e) => e.released.includes(t)).map((e) => e.step);
							expect(Math.max(-1, ...releasedAt)).toBeGreaterThanOrEqual(lastHanded.get(t)!);
						}
					}
					for (const i of [...open.keys()]) close(i);
				}
			)
		);
	});

	it('resolves false at once for no tickets', async () => {
		const { wait, calls } = manualWait();
		expect(await new RefillWaiter(wait, noSleep).landed([])).toBe(false);
		expect(calls).toHaveLength(0);
	});
});

describe('refetchOnRefill', () => {
	function landedWaiter(landed: boolean) {
		return new RefillWaiter(
			async (tickets) => tickets.map((ticket) => ({ ticket, landed })),
			noSleep
		);
	}

	it('fetches once more and applies it when a refill landed', async () => {
		const refetch = vi.fn(async () => 'filled');
		const apply = vi.fn();
		await refetchOnRefill(
			{ data: 'empty', refills: [1] },
			{ waiter: landedWaiter(true), refetch, apply }
		);
		expect(refetch).toHaveBeenCalledTimes(1);
		expect(apply).toHaveBeenCalledWith('filled');
	});

	it('does nothing for a read that waits on no refill', async () => {
		const refetch = vi.fn(async () => 'x');
		const apply = vi.fn();
		await refetchOnRefill(
			{ data: 'x', refills: [] },
			{ waiter: landedWaiter(true), refetch, apply }
		);
		expect(refetch).not.toHaveBeenCalled();
		expect(apply).not.toHaveBeenCalled();
	});

	it('does not refetch when the refill did not land', async () => {
		const refetch = vi.fn(async () => 'x');
		await refetchOnRefill(
			{ data: 'x', refills: [1] },
			{ waiter: landedWaiter(false), refetch, apply: vi.fn() }
		);
		expect(refetch).not.toHaveBeenCalled();
	});

	it('does not apply to a page closed while the refetch was out', async () => {
		const page = new AbortController();
		const apply = vi.fn();
		await refetchOnRefill(
			{ data: 'x', refills: [1] },
			{
				waiter: landedWaiter(true),
				refetch: async () => {
					page.abort();
					return 'late';
				},
				apply,
				signal: page.signal
			}
		);
		expect(apply).not.toHaveBeenCalled();
	});

	it('keeps what the page has when the refetch fails', async () => {
		const apply = vi.fn();
		await refetchOnRefill(
			{ data: 'x', refills: [1] },
			{
				waiter: landedWaiter(true),
				refetch: async () => {
					throw new Error('offline');
				},
				apply
			}
		);
		expect(apply).not.toHaveBeenCalled();
	});
});

describe('shareRefetch', () => {
	it('fetches once for every page refetching the same read at once', async () => {
		let calls = 0;
		const start = () => {
			calls += 1;
			return Promise.resolve('page');
		};
		const [a, b] = await Promise.all([shareRefetch('k', start), shareRefetch('k', start)]);
		expect([a, b]).toEqual(['page', 'page']);
		expect(calls).toBe(1);
	});

	it('fetches again once the shared read is done, and separately for another read', async () => {
		let calls = 0;
		const start = () => {
			calls += 1;
			return Promise.resolve(calls);
		};
		await shareRefetch('k2', start);
		await shareRefetch('k2', start);
		await shareRefetch('k3', start);
		expect(calls).toBe(3);
	});

	it('lets a failed read go, so the next asks again', async () => {
		await expect(shareRefetch('k4', () => Promise.reject(new Error('x')))).rejects.toThrow('x');
		await expect(shareRefetch('k4', () => Promise.resolve(1))).resolves.toBe(1);
	});
});
