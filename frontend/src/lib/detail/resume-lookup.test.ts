/**
 * The detail page starts its history lookup and its Kitsu detail
 * fetch together. A detail fetch that serves an entry Kitsu once
 * answered gone clears that mark, and the history lookup, being local,
 * usually answered before it did — with no row, since the mark hid
 * it. So an empty answer is asked again once the detail is served.
 */
import { describe, it, expect, vi } from 'vitest';
import fc from 'fast-check';
import { latestLookup, lookupResume } from './resume-lookup';

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
