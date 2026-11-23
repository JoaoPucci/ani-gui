import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { stripReads } from './strip-reads';

/** A read whose answer the test hands over when it chooses. */
function deferred<T>() {
	let answer!: (value: T) => void;
	let fail!: (e: unknown) => void;
	const promise = new Promise<T>((r, j) => ((answer = r), (fail = j)));
	return { promise, answer, fail };
}
const settle = () => new Promise((r) => setTimeout(r, 0));

/** A page: what it draws, and the re-reads it was asked for, in order. */
function harness(initial: string | null = null) {
	let drawn = initial;
	const rereads: { shown: string; read: ReturnType<typeof deferred<string | null>> }[] = [];
	const reads = stripReads<string>({
		current: () => drawn,
		reread: (shown) => {
			const read = deferred<string | null>();
			rereads.push({ shown, read });
			return read.promise;
		},
		show: (d) => (drawn = d)
	});
	return { reads, rereads, drawn: () => drawn };
}

describe('stripReads', () => {
	it("draws the visit's walk", async () => {
		const { reads, drawn } = harness();
		reads.walk(Promise.resolve('walked'));
		await settle();
		expect(drawn()).toBe('walked');
	});

	it('draws the rows a link answer reads again', async () => {
		const { reads, rereads, drawn } = harness();
		reads.walk(Promise.resolve('walked'));
		await settle();
		reads.linked();
		expect(rereads.map((r) => r.shown)).toEqual(['walked']);
		rereads[0].read.answer('after the link');
		await settle();
		expect(drawn()).toBe('after the link');
	});

	it('keeps the latest of two links, whichever re-read lands first', async () => {
		const { reads, rereads, drawn } = harness();
		reads.walk(Promise.resolve('walked'));
		await settle();
		reads.linked();
		reads.linked();
		rereads[0].read.answer('after link A');
		await settle();
		rereads[1].read.answer('after links A and B');
		await settle();
		expect(drawn()).toBe('after links A and B');
		// And the earlier one landing last does not undo it.
		const late = harness();
		late.reads.walk(Promise.resolve('walked'));
		await settle();
		late.reads.linked();
		late.reads.linked();
		late.rereads[1].read.answer('after links A and B');
		await settle();
		late.rereads[0].read.answer('after link A');
		await settle();
		expect(late.drawn()).toBe('after links A and B');
	});

	it('reads the rows again once a walk still out lands, for a link answered meanwhile', async () => {
		// A revisit draws its remembered answer at once and walks again
		// behind it; the walk's rows may have been read before the link.
		const { reads, rereads, drawn } = harness('remembered');
		const walk = deferred<string | null>();
		reads.walk(walk.promise);
		reads.linked();
		expect(rereads).toEqual([]);
		walk.answer('walked');
		await settle();
		expect(rereads.map((r) => r.shown)).toEqual(['walked']);
		rereads[0].read.answer('after the link');
		await settle();
		expect(drawn()).toBe('after the link');
	});

	it('reads the rows again for a link answered while a walk that fails was out', async () => {
		const { reads, rereads } = harness('remembered');
		const walk = deferred<string | null>();
		reads.walk(walk.promise);
		reads.linked();
		walk.fail(new Error('kitsu unreachable'));
		await settle();
		expect(rereads.map((r) => r.shown)).toEqual(['remembered']);
	});

	it("drops a re-read once the next visit's walk starts", async () => {
		const { reads, rereads, drawn } = harness();
		reads.walk(Promise.resolve('show A'));
		await settle();
		reads.linked();
		reads.walk(Promise.resolve('show B'));
		await settle();
		rereads[0].read.answer('show A after the link');
		await settle();
		expect(drawn()).toBe('show B');
	});

	it('reads nothing again with nothing drawn and no walk out', async () => {
		const { reads, rereads } = harness();
		reads.linked();
		expect(rereads).toEqual([]);
	});

	it('draws what the last re-read started answers, whatever order they land in', async () => {
		await fc.assert(
			fc.asyncProperty(fc.integer({ min: 1, max: 6 }), fc.func(fc.boolean()), async (n, early) => {
				const { reads, rereads, drawn } = harness();
				reads.walk(Promise.resolve('walked'));
				await settle();
				for (let i = 0; i < n; i++) reads.linked();
				const order = [...rereads.keys()].sort((a, b) => Number(early(a)) - Number(early(b)));
				for (const i of order) {
					rereads[i].read.answer(`re-read ${i}`);
					await settle();
				}
				expect(drawn()).toBe(`re-read ${n - 1}`);
			})
		);
	});
});
