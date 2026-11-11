import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { createFranchiseReads, createNamedReads } from './handoff-reads';
import type { FranchiseData } from './types';

const franchise = (anchor: string): FranchiseData => ({
	anchor,
	entries: [],
	links: [],
	versions: [],
	films: [],
	sides: [],
	recaps: [],
	parents: {},
	complete: true
});

const flush = () => new Promise((r) => setTimeout(r, 0));

describe('createFranchiseReads', () => {
	it('reads a series once however often it is wanted, and hands it on', async () => {
		const read = vi.fn(async (id: string) => franchise(id));
		const settled = vi.fn();
		const reads = createFranchiseReads(read, settled);
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
		const reads = createFranchiseReads(read, settled);
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
		const reads = createFranchiseReads(read, settled);
		reads.want('7');
		await flush();
		reads.want('7');
		await flush();
		expect(read).toHaveBeenCalledTimes(1);
		expect(settled).toHaveBeenCalledWith('7', null);
	});
});

describe('createFranchiseReads, when the page goes', () => {
	it('cancels every read still out, and hands on no answer after', async () => {
		const signals: AbortSignal[] = [];
		const answers: Array<(f: FranchiseData) => void> = [];
		const read = (id: string, signal: AbortSignal) => {
			void id;
			signals.push(signal);
			return new Promise<FranchiseData>((r) => answers.push(r));
		};
		const settled = vi.fn();
		const reads = createFranchiseReads(read, settled);
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
		const reads = createFranchiseReads(
			(id, signal) => (signals.push(signal), new Promise<FranchiseData>(() => {})),
			vi.fn()
		);
		reads.want('7');
		expect(signals[0].aborted).toBe(false);
	});
});

describe('createFranchiseReads, on the franchise lane', () => {
	// The walks have the franchise lane's six connections to themselves:
	// Home's own calls, a Continue card's resume among them, go to the
	// API lane.
	const AT_ONCE = 6;
	const ids = (n: number) => Array.from({ length: n }, (_, i) => String(i + 1));

	/** Reads that answer when the test says, with how many are out. */
	function held() {
		const out = new Map<string, (f: FranchiseData | Error) => void>();
		const started: string[] = [];
		const read = (id: string, signal: AbortSignal) => {
			started.push(id);
			return new Promise<FranchiseData>((resolve, reject) => {
				const end = (f: FranchiseData | Error) => {
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
		const reads = createFranchiseReads(read, vi.fn());
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
		const reads = createFranchiseReads(read, settled);
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
					const reads = createFranchiseReads(read, settled);
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

describe('createNamedReads, when the page goes or the history changes', () => {
	const hanging = () => {
		const signals: AbortSignal[] = [];
		const read = (ids: readonly string[], signal: AbortSignal) => {
			void ids;
			signals.push(signal);
			return new Promise<Record<string, unknown>>(() => {});
		};
		return { signals, read };
	};

	it('cancels the lookups still out when the page goes', () => {
		const { signals, read } = hanging();
		const reads = createNamedReads(read, vi.fn());
		reads.want('20');
		reads.want('30');
		reads.stop();
		expect(signals.map((s) => s.aborted)).toEqual([true, true]);
	});

	it('cancels the lookups whose answers forgetting drops, and leaves the next ones running', () => {
		const { signals, read } = hanging();
		const reads = createNamedReads(read, vi.fn());
		reads.want('20');
		reads.forget();
		reads.want('20');
		expect(signals.map((s) => s.aborted)).toEqual([true, false]);
	});

	it('hands on no answer once stopped', async () => {
		const settled = vi.fn();
		let answer!: (rows: Record<string, unknown>) => void;
		const reads = createNamedReads(() => new Promise((r) => (answer = r)), settled);
		reads.want('20');
		reads.stop();
		answer({ '20': { ep_no: '1' } });
		await flush();
		expect(settled).not.toHaveBeenCalled();
	});
});

describe('createNamedReads', () => {
	it('asks the resume lookup about an id once, and says whether a row resumes it', async () => {
		const read = vi.fn(async (ids: readonly string[]) =>
			Object.fromEntries(ids.map((id) => [id, id === '20' ? { ep_no: '3' } : null]))
		);
		const settled = vi.fn();
		const reads = createNamedReads(read, settled);
		reads.want('20');
		reads.want('20');
		reads.want('30');
		await flush();
		reads.want('20');
		await flush();
		expect(read).toHaveBeenCalledTimes(2);
		expect(read.mock.calls[0][0]).toEqual(['20']);
		expect(settled).toHaveBeenCalledWith('20', true);
		expect(settled).toHaveBeenCalledWith('30', false);
	});

	it('settles a lookup that fails as unknown', async () => {
		const settled = vi.fn();
		const reads = createNamedReads(() => Promise.reject(new Error('no backend')), settled);
		reads.want('20');
		await flush();
		expect(settled).toHaveBeenCalledWith('20', null);
	});

	it('asks again after forgetting, and drops an answer still out from before', async () => {
		let answer!: (rows: Record<string, unknown>) => void;
		const read = vi
			.fn<(ids: readonly string[]) => Promise<Record<string, unknown>>>()
			.mockImplementationOnce(() => new Promise((r) => (answer = r)))
			.mockImplementation(async () => ({ '20': { ep_no: '1' } }));
		const settled = vi.fn();
		const reads = createNamedReads(read, settled);
		reads.want('20');
		reads.forget();
		reads.want('20');
		await flush();
		answer({ '20': null });
		await flush();
		expect(read).toHaveBeenCalledTimes(2);
		expect(settled.mock.calls).toEqual([['20', true]]);
	});
});
