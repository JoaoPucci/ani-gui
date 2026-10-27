import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { beginOwnWrite, ownWrites } from './own-writes';

describe('own writes', () => {
	it('holds a bearer from the start of its save until the save ends', () => {
		const end = beginOwnWrite('mal', 'b');
		expect(ownWrites('mal').has('b')).toBe(true);
		expect(ownWrites('anilist').has('b')).toBe(false);
		end();
		expect(ownWrites('mal').has('b')).toBe(false);
	});

	it('keeps a bearer while any of its saves is out, and ends each save once', () => {
		fc.assert(
			fc.property(fc.array(fc.boolean(), { minLength: 1, maxLength: 8 }), (twice) => {
				const ends = twice.map(() => beginOwnWrite('mal', 'x'));
				ends.forEach((end, i) => {
					end();
					if (twice[i]) end();
					const left = i < ends.length - 1;
					if (ownWrites('mal').has('x') !== left) throw new Error(`after ${i}`);
				});
				return !ownWrites('mal').has('x');
			})
		);
	});
});
