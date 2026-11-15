import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { latestRead } from './latest-read';

/** A read whose answer the test hands over when it chooses. */
function deferred<T>() {
	let answer!: (value: T) => void;
	const promise = new Promise<T>((r) => (answer = r));
	return { promise, answer };
}
const settle = () => new Promise((r) => setTimeout(r, 0));

function harness() {
	const shown: (string | null)[] = [];
	const reads = latestRead<string>((v) => shown.push(v));
	const now = () => shown.at(-1) ?? null;
	return { shown, reads, now };
}

describe('latestRead', () => {
	it('shows nothing while a read is out, then its answer', async () => {
		const { reads, now, shown } = harness();
		const a = deferred<string>();
		reads.start(() => a.promise);
		expect(shown).toEqual([null]);
		a.answer('show A');
		await settle();
		expect(now()).toBe('show A');
	});

	it('drops the answer of a show left before it landed', async () => {
		const { reads, now } = harness();
		const a = deferred<string>();
		const b = deferred<string>();
		reads.start(() => a.promise);
		reads.start(() => b.promise);
		b.answer('show B');
		await settle();
		a.answer('show A, late');
		await settle();
		expect(now()).toBe('show B');
	});

	it('keeps a Try again from being replaced by the failed read it retried', async () => {
		const { reads, now } = harness();
		const first = deferred<string>();
		const retry = deferred<string>();
		reads.start(() => first.promise);
		reads.start(() => retry.promise);
		retry.answer('read');
		first.answer('failed');
		await settle();
		expect(now()).toBe('read');
	});

	it('shows nothing once stopped, and drops a read still out', async () => {
		const { reads, now, shown } = harness();
		const a = deferred<string>();
		reads.start(() => a.promise);
		reads.stop();
		expect(now()).toBeNull();
		a.answer('show A');
		await settle();
		expect(now()).toBeNull();
		expect(shown).toEqual([null, null]);
	});

	it('cancels a read it supersedes, and leaves the new one running', () => {
		const { reads } = harness();
		const signals: AbortSignal[] = [];
		reads.start((signal) => (signals.push(signal), new Promise<string>(() => {})));
		reads.start((signal) => (signals.push(signal), new Promise<string>(() => {})));
		expect(signals.map((s) => s.aborted)).toEqual([true, false]);
	});

	it('cancels the read still out when stopped', () => {
		const { reads } = harness();
		let signal: AbortSignal | null = null;
		reads.start((s) => ((signal = s), new Promise<string>(() => {})));
		reads.stop();
		expect(signal!.aborted).toBe(true);
	});

	it('leaves a read that answered alone', async () => {
		const { reads, now } = harness();
		let signal: AbortSignal | null = null;
		reads.start((s) => ((signal = s), Promise.resolve('show A')));
		await settle();
		expect(now()).toBe('show A');
		expect(signal!.aborted).toBe(false);
	});

	it('keeps only the latest unstopped read uncancelled, through any run of starts and stops', () => {
		fc.assert(
			fc.property(fc.array(fc.boolean(), { maxLength: 12 }), (ops) => {
				const { reads } = harness();
				const signals: AbortSignal[] = [];
				let live: number | null = null;
				for (const isStart of ops) {
					if (isStart) {
						reads.start((s) => (signals.push(s), new Promise<string>(() => {})));
						live = signals.length - 1;
					} else {
						reads.stop();
						live = null;
					}
					expect(signals.map((s) => s.aborted)).toEqual(signals.map((_, i) => i !== live));
				}
			})
		);
	});

	it('shows only the latest read through any run of starts, stops and answers', async () => {
		type Op = { kind: 'start' } | { kind: 'stop' } | { kind: 'answer'; pick: number };
		const opArb: fc.Arbitrary<Op> = fc.oneof(
			fc.constant({ kind: 'start' as const }),
			fc.constant({ kind: 'stop' as const }),
			fc.nat().map((pick) => ({ kind: 'answer' as const, pick }))
		);
		await fc.assert(
			fc.asyncProperty(fc.array(opArb, { maxLength: 12 }), async (ops) => {
				const { reads, now } = harness();
				const pending: { d: ReturnType<typeof deferred<string>>; n: number }[] = [];
				let started = 0;
				// The read whose answer may be shown: the latest start, unless a
				// stop came after it.
				let live: number | null = null;
				const answered = new Set<number>();
				for (const op of ops) {
					if (op.kind === 'start') {
						const d = deferred<string>();
						const n = started++;
						pending.push({ d, n });
						reads.start(() => d.promise);
						live = n;
						expect(now()).toBeNull();
					} else if (op.kind === 'stop') {
						reads.stop();
						live = null;
						expect(now()).toBeNull();
					} else if (pending.length > 0) {
						const [{ d, n }] = pending.splice(op.pick % pending.length, 1);
						d.answer(`read ${n}`);
						await settle();
						answered.add(n);
					}
					// Shown: the live read's answer once it has landed, else nothing.
					expect(now()).toBe(live !== null && answered.has(live) ? `read ${live}` : null);
				}
			}),
			{ numRuns: 100 }
		);
	});
});
