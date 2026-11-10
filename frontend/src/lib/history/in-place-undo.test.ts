// An Undo from Home's notice, made after the surface may have moved on:
// what follows the rows given back, for each place the user can be.

import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { surface, undoInPlace } from './in-place-undo';

/** Where the session is as the Undo is pressed, and as it answers. */
interface Place {
	row: string | null;
	arrival: number;
	gone: boolean;
}

/** Run an Undo of the card `row`'s link from `before` to `after`; the
 *  steps it took, in order. */
async function run(row: string, before: Place, after: Place): Promise<string[]> {
	const steps: string[] = [];
	const place = surface();
	let now = before;
	if (before.gone) place.leave();
	await undoInPlace({
		row,
		surface: place,
		at: () => ({ row: now.row, arrival: now.arrival }),
		clear: () => steps.push('clear'),
		undo: async () => {
			steps.push('undo');
			now = after;
			if (after.gone) place.leave();
		},
		refresh: () => steps.push('refresh'),
		offerAgain: async () => steps.push('offer again')
	});
	return steps;
}

const on = (row: string | null, arrival = 1): Place => ({ row, arrival, gone: false });

describe('an Undo from an in-place surface', () => {
	it('on the same card throughout clears its note, then reads again and offers it again', async () => {
		expect(await run('a', on('a'), on('a'))).toEqual(['clear', 'undo', 'refresh', 'offer again']);
	});

	it('once another card arrived reads again and leaves that card alone', async () => {
		expect(await run('a', on('b', 2), on('b', 2))).toEqual(['undo', 'refresh']);
		expect(await run('a', on('a'), on('b', 2))).toEqual(['clear', 'undo', 'refresh']);
	});

	it('on the same card arrived again meanwhile leaves that arrival its own', async () => {
		expect(await run('a', on('a'), on('a', 2))).toEqual(['clear', 'undo', 'refresh']);
	});

	it('once the surface is gone gives the rows back and nothing else', async () => {
		const gone = { row: 'a', arrival: 1, gone: true };
		expect(await run('a', gone, gone)).toEqual(['undo']);
		expect(await run('a', on('a'), gone)).toEqual(['clear', 'undo']);
	});

	it('always gives the rows back once; reads again unless gone; offers again only on its own visit', async () => {
		const place = fc.record({
			row: fc.constantFrom('a', 'b', null),
			arrival: fc.integer({ min: 1, max: 3 }),
			gone: fc.boolean()
		});
		await fc.assert(
			fc.asyncProperty(place, place, async (before, raw) => {
				// A surface once left stays left.
				const after = { ...raw, gone: raw.gone || before.gone };
				const steps = await run('a', before, after);
				const own = (p: Place) => !p.gone && p.row === 'a';
				expect(steps.filter((s) => s === 'undo')).toHaveLength(1);
				expect(steps.includes('clear')).toBe(own(before));
				expect(steps.includes('refresh')).toBe(!after.gone);
				expect(steps.includes('offer again')).toBe(own(after) && after.arrival === before.arrival);
			})
		);
	});
});

describe('the surface', () => {
	it('is here until it is left, and somewhere else after', () => {
		const place = surface();
		expect(place.gone).toBe(false);
		const here = place.here();
		place.leave();
		expect(place.gone).toBe(true);
		expect(place.here()).not.toBe(here);
	});
});
