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

	it('shows only the latest read, whatever order the answers land in', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.integer({ min: 1, max: 6 }),
				fc.array(fc.nat(), { minLength: 6, maxLength: 6 }),
				fc.boolean(),
				async (count, order, stopAtEnd) => {
					const { reads, now } = harness();
					const pending = Array.from({ length: count }, () => deferred<string>());
					pending.forEach((d) => reads.start(() => d.promise));
					if (stopAtEnd) reads.stop();
					// Answer every read, in an order the arbitrary picks.
					const left = pending.map((d, i) => ({ d, i }));
					for (const n of order) {
						if (left.length === 0) break;
						const [{ d, i }] = left.splice(n % left.length, 1);
						d.answer(`read ${i}`);
						await settle();
						if (stopAtEnd) expect(now()).toBeNull();
						else if (i === count - 1) expect(now()).toBe(`read ${i}`);
						else expect([null, `read ${count - 1}`]).toContain(now());
					}
					for (const { d, i } of left) d.answer(`read ${i}`);
					await settle();
					expect(now()).toBe(stopAtEnd ? null : `read ${count - 1}`);
				}
			),
			{ numRuns: 50 }
		);
	});
});
