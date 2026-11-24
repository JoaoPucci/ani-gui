import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import type { Handoff } from '$lib/api';
import {
	createHandoffReads,
	handoffWalksAtOnce,
	HANDOFF_WALKS_ON_OWN_LANE,
	HANDOFF_WALKS_ON_SHARED_LANE
} from './handoff-reads';
import type { WatchEntry } from './types';

const entry = (id: string): WatchEntry => ({
	id,
	title: `Entry ${id}`,
	subtype: 'TV',
	start: '2020-01-01',
	episodes: 12,
	status: 'finished',
	poster: null,
	cover: null
});
/** The hand-off read for `id`: on to the entry after it. */
const franchise = (id: string): Handoff => ({ finished: entry(id), next: entry(`${id}0`) });

const flush = () => new Promise((r) => setTimeout(r, 0));

describe('createHandoffReads', () => {
	it('reads a series once however often it is wanted, and hands it on', async () => {
		const read = vi.fn(async (id: string) => franchise(id));
		const settled = vi.fn();
		const reads = createHandoffReads(read, settled, HANDOFF_WALKS_ON_OWN_LANE);
		reads.want('7');
		reads.want('7');
		await flush();
		reads.want('7');
		await flush();
		expect(read).toHaveBeenCalledTimes(1);
		expect(settled).toHaveBeenCalledTimes(1);
		expect(settled).toHaveBeenCalledWith('7', franchise('7'));
	});

	it('reads each series it is asked for', async () => {
		const read = vi.fn(async (id: string) => franchise(id));
		const settled = vi.fn();
		const reads = createHandoffReads(read, settled, HANDOFF_WALKS_ON_OWN_LANE);
		reads.want('7');
		reads.want('8');
		await flush();
		expect(read.mock.calls.map(([id]) => id)).toEqual(['7', '8']);
		expect(settled).toHaveBeenCalledWith('8', franchise('8'));
	});

	it('settles a failed read as nothing, without asking again', async () => {
		const read = vi.fn(async () => {
			throw new Error('503');
		});
		const settled = vi.fn();
		const reads = createHandoffReads(read, settled, HANDOFF_WALKS_ON_OWN_LANE);
		reads.want('7');
		await flush();
		reads.want('7');
		await flush();
		expect(read).toHaveBeenCalledTimes(1);
		expect(settled).toHaveBeenCalledWith('7', null);
	});
});

describe('createHandoffReads, when the page goes', () => {
	it('cancels every read still out, and hands on no answer after', async () => {
		const signals: AbortSignal[] = [];
		const answers: Array<(f: Handoff) => void> = [];
		const read = (id: string, signal: AbortSignal) => {
			void id;
			signals.push(signal);
			return new Promise<Handoff>((r) => answers.push(r));
		};
		const settled = vi.fn();
		const reads = createHandoffReads(read, settled, HANDOFF_WALKS_ON_OWN_LANE);
		reads.want('7');
		reads.want('8');
		reads.stop();
		expect(signals.map((s) => s.aborted)).toEqual([true, true]);
		answers[0](franchise('7'));
		await flush();
		expect(settled).not.toHaveBeenCalled();
	});

	it('leaves the reads running until then', () => {
		const signals: AbortSignal[] = [];
		const reads = createHandoffReads(
			(id, signal) => (signals.push(signal), new Promise<Handoff>(() => {})),
			vi.fn(),
			HANDOFF_WALKS_ON_OWN_LANE
		);
		reads.want('7');
		expect(signals[0].aborted).toBe(false);
	});
});

describe('handoffWalksAtOnce', () => {
	it('walks six at once when the franchise lane is an origin of its own', () => {
		expect(handoffWalksAtOnce(true)).toBe(HANDOFF_WALKS_ON_OWN_LANE);
		expect(HANDOFF_WALKS_ON_OWN_LANE).toBe(6);
	});

	it("walks two at once when franchise reads share the API's origin", () => {
		// Browser-only dev, or a window opened without the lane: the walks
		// would otherwise take all six of the API origin's connections, and
		// a Continue card's resume or a removal would queue behind them.
		expect(handoffWalksAtOnce(false)).toBe(HANDOFF_WALKS_ON_SHARED_LANE);
		expect(HANDOFF_WALKS_ON_SHARED_LANE).toBe(2);
	});
});

describe('createHandoffReads, on the franchise lane', () => {
	// The walks have the franchise lane's six connections to themselves:
	// Home's own calls, a Continue card's resume among them, go to the
	// API lane.
	const AT_ONCE = HANDOFF_WALKS_ON_OWN_LANE;
	const ids = (n: number) => Array.from({ length: n }, (_, i) => String(i + 1));

	/** Reads that answer when the test says, with how many are out. */
	function held() {
		const out = new Map<string, (f: Handoff | Error) => void>();
		const started: string[] = [];
		const read = (id: string, signal: AbortSignal) => {
			started.push(id);
			return new Promise<Handoff>((resolve, reject) => {
				const end = (f: Handoff | Error) => {
					out.delete(id);
					if (f instanceof Error) reject(f);
					else resolve(f);
				};
				out.set(id, end);
				signal.addEventListener('abort', () => end(new Error('aborted')), { once: true });
			});
		};
		return { out, started, read };
	}

	it(`walks at most ${AT_ONCE} series at once, and the next as one answers`, async () => {
		const { out, started, read } = held();
		const reads = createHandoffReads(read, vi.fn(), HANDOFF_WALKS_ON_OWN_LANE);
		for (const id of ids(8)) reads.want(id);
		expect(started).toEqual(ids(6));
		out.get('1')!(franchise('1'));
		await flush();
		expect(started).toEqual(ids(7));
		out.get('2')!(new Error('503'));
		await flush();
		expect(started).toEqual(ids(8));
	});

	it('starts none of the waiting reads once the page has gone', async () => {
		const { out, started, read } = held();
		const settled = vi.fn();
		const reads = createHandoffReads(read, settled, HANDOFF_WALKS_ON_OWN_LANE);
		for (const id of ids(8)) reads.want(id);
		reads.stop();
		await flush();
		expect(started).toEqual(ids(6));
		expect(out.size).toBe(0);
		expect(settled).not.toHaveBeenCalled();
	});

	it('reads every series wanted exactly once, never more than the share at a time', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.array(fc.constantFrom(...ids(10)), { maxLength: 16 }),
				fc.array(fc.tuple(fc.nat(), fc.boolean()), { maxLength: 12 }),
				async (wants, answers) => {
					const { out, started, read } = held();
					const settled = vi.fn();
					const reads = createHandoffReads(read, settled, HANDOFF_WALKS_ON_OWN_LANE);
					let peak = 0;
					const watch = () => (peak = Math.max(peak, out.size));
					for (const id of wants) {
						reads.want(id);
						watch();
					}
					for (const [pick, ok] of answers) {
						const ids = [...out.keys()];
						if (ids.length === 0) break;
						const id = ids[pick % ids.length];
						out.get(id)!(ok ? franchise(id) : new Error('503'));
						await flush();
						watch();
					}
					// Answer whatever is left.
					while (out.size > 0) {
						const [id, end] = [...out.entries()][0];
						end(franchise(id));
						await flush();
						watch();
					}
					const unique = [...new Set(wants)];
					expect(peak).toBeLessThanOrEqual(AT_ONCE);
					expect(started).toEqual(unique);
					expect(settled.mock.calls.map(([id]) => id).sort()).toEqual([...unique].sort());
				}
			)
		);
	});
});

describe('createHandoffReads, sharing the API origin', () => {
	it('keeps to the budget it is given, and starts the next as one answers', async () => {
		const out = new Map<string, (f: Handoff) => void>();
		const started: string[] = [];
		const read = (id: string) => {
			started.push(id);
			return new Promise<Handoff>((resolve) => out.set(id, resolve));
		};
		const reads = createHandoffReads(read, vi.fn(), HANDOFF_WALKS_ON_SHARED_LANE);
		for (const id of ['1', '2', '3', '4']) reads.want(id);
		expect(started).toEqual(['1', '2']);
		out.get('1')!(franchise('1'));
		await flush();
		expect(started).toEqual(['1', '2', '3']);
	});
});

describe('createHandoffReads, when the history changes', () => {
	const hanging = () => {
		const signals: AbortSignal[] = [];
		const read = (id: string, signal: AbortSignal) => {
			void id;
			signals.push(signal);
			return new Promise<Handoff | null>(() => {});
		};
		return { signals, read };
	};

	it('cancels the reads whose answers forgetting drops, and leaves the next ones running', () => {
		const { signals, read } = hanging();
		const reads = createHandoffReads(read, vi.fn(), HANDOFF_WALKS_ON_OWN_LANE);
		reads.want('20');
		reads.forget();
		reads.want('20');
		expect(signals.map((s) => s.aborted)).toEqual([true, false]);
	});

	it('asks again after forgetting, and drops an answer still out from before', async () => {
		let answer!: (h: Handoff | null) => void;
		const read = vi
			.fn<(id: string, signal: AbortSignal) => Promise<Handoff | null>>()
			.mockImplementationOnce(() => new Promise((r) => (answer = r)))
			.mockImplementation(async (id) => franchise(id));
		const settled = vi.fn();
		const reads = createHandoffReads(read, settled, HANDOFF_WALKS_ON_OWN_LANE);
		reads.want('20');
		reads.forget();
		reads.want('20');
		await flush();
		answer(null);
		await flush();
		expect(read).toHaveBeenCalledTimes(2);
		expect(settled.mock.calls).toEqual([['20', franchise('20')]]);
	});

	it('drops the reads still waiting for the budget, and reads only what is wanted after', async () => {
		const started: string[] = [];
		const read = (id: string, signal: AbortSignal) => {
			started.push(id);
			return new Promise<Handoff | null>((_, reject) =>
				signal.addEventListener('abort', () => reject(new Error('aborted')), { once: true })
			);
		};
		const reads = createHandoffReads(read, vi.fn(), HANDOFF_WALKS_ON_SHARED_LANE);
		for (const id of ['1', '2', '3', '4']) reads.want(id);
		expect(started).toEqual(['1', '2']);
		reads.forget();
		reads.want('5');
		await flush();
		expect(started).toEqual(['1', '2', '5']);
	});

	it('does nothing once stopped', async () => {
		const read = vi.fn(async (id: string) => franchise(id));
		const reads = createHandoffReads(read, vi.fn(), HANDOFF_WALKS_ON_OWN_LANE);
		reads.stop();
		reads.forget();
		reads.want('7');
		await flush();
		expect(read).not.toHaveBeenCalled();
	});
});
