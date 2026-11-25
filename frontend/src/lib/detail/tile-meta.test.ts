import fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { tileMeta, type TileState } from './tile-meta';

const tile = (over: Partial<TileState> = {}): TileState => ({
	unaired: false,
	hasThumb: true,
	notListed: false,
	length: 24,
	...over
});

describe('tileMeta', () => {
	it("gives a playable episode's length", () => {
		expect(tileMeta(tile())).toEqual({ kind: 'length', minutes: 24 });
	});

	it('says an episode the catalogue does not list yet, in place of its length', () => {
		// A state the tile shows only by dimming would be lost where
		// colours are not the app's (a contrast theme): it is said in words.
		expect(tileMeta(tile({ notListed: true }))).toEqual({ kind: 'not-listed' });
		expect(tileMeta(tile({ notListed: true, hasThumb: false }))).toEqual({ kind: 'not-listed' });
	});

	it("gives an unaired episode's airing under a picture, and leaves it to the placeholder without one", () => {
		expect(tileMeta(tile({ unaired: true }))).toEqual({ kind: 'unaired' });
		expect(tileMeta(tile({ unaired: true, hasThumb: false }))).toBeNull();
	});

	it('says nothing for a playable episode of unknown length', () => {
		expect(tileMeta(tile({ length: null }))).toBeNull();
	});

	it('never says an unaired episode is unlisted, nor gives a length for either', () => {
		fc.assert(
			fc.property(
				fc.boolean(),
				fc.boolean(),
				fc.boolean(),
				fc.option(fc.integer({ min: 1, max: 200 }), { nil: null }),
				(unaired, hasThumb, notListed, length) => {
					const meta = tileMeta({ unaired, hasThumb, notListed, length });
					if (unaired) expect(meta?.kind ?? null).not.toBe('not-listed');
					if (unaired || notListed) expect(meta?.kind ?? null).not.toBe('length');
					if (!unaired && notListed) expect(meta).toEqual({ kind: 'not-listed' });
				}
			)
		);
	});
});
