/**
 * The detail page starts its history lookup and its Kitsu detail
 * fetch together. A detail fetch that serves an entry Kitsu once
 * answered gone clears that mark, and the history lookup, being local,
 * usually answered before it did — with no row, since the mark hid
 * it. So an empty answer is asked again once the detail is served.
 */
import { describe, it, expect, vi } from 'vitest';
import fc from 'fast-check';
import { latestLookup, lookupResume, readResume } from './resume-lookup';
import { beginVisit } from '$lib/navigation/visit';

const never = new Promise<never>(() => {});

describe('lookupResume', () => {
	it('answers a found row without waiting on the detail fetch', async () => {
		const lookup = vi.fn().mockResolvedValue({ id: 'row' });
		await expect(lookupResume('42', lookup, never)).resolves.toEqual({ id: 'row' });
		expect(lookup).toHaveBeenCalledTimes(1);
	});

	it('asks again once the detail is served when the first answer was empty', async () => {
		const lookup = vi.fn().mockResolvedValueOnce(null).mockResolvedValueOnce({ id: 'row' });
		await expect(lookupResume('42', lookup, Promise.resolve({}))).resolves.toEqual({
			id: 'row'
		});
		expect(lookup).toHaveBeenCalledTimes(2);
		expect(lookup).toHaveBeenNthCalledWith(2, '42');
	});

	it('answers empty when the second ask finds nothing either', async () => {
		const lookup = vi.fn().mockResolvedValue(null);
		await expect(lookupResume('42', lookup, Promise.resolve({}))).resolves.toBeNull();
		expect(lookup).toHaveBeenCalledTimes(2);
	});

	it('does not ask again when the detail fetch fails', async () => {
		const lookup = vi.fn().mockResolvedValue(null);
		await expect(lookupResume('42', lookup, Promise.reject(new Error('404')))).resolves.toBeNull();
		expect(lookup).toHaveBeenCalledTimes(1);
	});
});

describe('latestLookup', () => {
	it('answers for the latest lookup started, and no earlier one', () => {
		const start = latestLookup();
		const first = start();
		expect(first()).toBe(true);
		const second = start();
		expect(first()).toBe(false);
		expect(second()).toBe(true);
	});

	it('holds for exactly the last start of any run of starts', () => {
		fc.assert(
			fc.property(fc.integer({ min: 1, max: 20 }), (n) => {
				const start = latestLookup();
				const tickets = Array.from({ length: n }, () => start());
				expect(tickets.map((t) => t())).toEqual(tickets.map((_, i) => i === n - 1));
			})
		);
	});
});

describe('readResume', () => {
	const flush = () => new Promise((r) => setTimeout(r, 0));

	it("reads under the show's visit and shows the row found", async () => {
		const reads = beginVisit();
		const lookup = vi.fn(async () => 'row');
		const apply = vi.fn();
		readResume({
			kitsuId: '7',
			reads,
			latest: () => true,
			lookup,
			detailServed: Promise.resolve(),
			showing: () => '7',
			apply
		});
		await flush();
		expect(lookup).toHaveBeenCalledWith('7', { signal: reads.signal });
		expect(apply).toHaveBeenCalledWith('row');
	});

	it('shows nothing for a show the page moved on from, a later lookup, or an ended visit', async () => {
		for (const [showing, latest, end] of [
			['8', true, false],
			['7', false, false],
			['7', true, true]
		] as const) {
			const reads = beginVisit();
			const apply = vi.fn();
			readResume({
				kitsuId: '7',
				reads,
				latest: () => latest,
				lookup: async () => 'row',
				detailServed: Promise.resolve(),
				showing: () => showing,
				apply
			});
			if (end) reads.end();
			await flush();
			expect(apply).not.toHaveBeenCalled();
		}
	});

	it('leaves no failure of a detail fetch it never waited for unhandled', async () => {
		// A row found at once never waits for the detail; the detail's
		// fetch, cancelled with the show's visit, rejects all the same.
		const unhandled: unknown[] = [];
		const note = (e: unknown) => unhandled.push(e);
		process.on('unhandledRejection', note);
		try {
			const reads = beginVisit();
			readResume({
				kitsuId: '7',
				reads,
				latest: () => true,
				lookup: async () => 'row',
				detailServed: Promise.reject(new DOMException('The operation was aborted.', 'AbortError')),
				showing: () => '7',
				apply: () => {}
			});
			await new Promise((r) => setTimeout(r, 20));
			expect(unhandled).toEqual([]);
		} finally {
			process.off('unhandledRejection', note);
		}
	});

	it('drops a failure: resume is a hint', async () => {
		const apply = vi.fn();
		readResume({
			kitsuId: '7',
			reads: beginVisit(),
			latest: () => true,
			lookup: async () => {
				throw new Error('history unreadable');
			},
			detailServed: Promise.resolve(),
			showing: () => '7',
			apply
		});
		await flush();
		expect(apply).not.toHaveBeenCalled();
	});
});
