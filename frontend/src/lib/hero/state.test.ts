import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { heroState } from './state';

const shows = ['a', 'b', 'c'];

describe('heroState', () => {
	it('is loading while the trending list has not arrived', () => {
		expect(heroState({ rotation: null, failed: false, index: 0 })).toEqual({ kind: 'loading' });
	});

	it('features the show the index reaches, wrapping around the rotation', () => {
		expect(heroState({ rotation: shows, failed: false, index: 0 })).toEqual({
			kind: 'featured',
			show: 'a'
		});
		expect(heroState({ rotation: shows, failed: false, index: 4 })).toEqual({
			kind: 'featured',
			show: 'b'
		});
	});

	it('says the fetch failed when it did', () => {
		expect(heroState({ rotation: null, failed: true, index: 0 })).toEqual({ kind: 'failed' });
	});

	it('says the list came back empty once it has, rather than loading on', () => {
		expect(heroState({ rotation: [], failed: false, index: 0 })).toEqual({ kind: 'empty' });
		expect(heroState({ rotation: [], failed: false, index: 7 })).toEqual({ kind: 'empty' });
	});

	it('reports a failed fetch over an empty rotation', () => {
		expect(heroState({ rotation: [], failed: true, index: 0 })).toEqual({ kind: 'failed' });
	});

	it('is loading only while the list has not arrived and the fetch has not failed', () => {
		fc.assert(
			fc.property(
				fc.option(fc.array(fc.string(), { maxLength: 6 })),
				fc.nat(50),
				fc.boolean(),
				(rotation, index, failed) => {
					const s = heroState({ rotation, failed, index });
					expect(s.kind === 'loading').toBe(rotation === null && !failed);
				}
			)
		);
	});

	it('features a show of the rotation for any non-empty rotation and index', () => {
		fc.assert(
			fc.property(
				fc.array(fc.string(), { minLength: 1, maxLength: 6 }),
				fc.nat(50),
				fc.boolean(),
				(rotation, index, failed) => {
					const s = heroState({ rotation, failed, index });
					expect(s.kind).toBe('featured');
					if (s.kind === 'featured') expect(s.show).toBe(rotation[index % rotation.length]);
				}
			)
		);
	});
});
