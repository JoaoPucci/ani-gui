import { describe, expect, it } from 'vitest';
import { levelMapping, remapHold, remapIndex, remapKeys } from './quality-remap';

const LEVELS = [
	{ height: 360, bitrate: 800_000 },
	{ height: 720, bitrate: 2_500_000 },
	{ height: 1080, bitrate: 5_000_000 }
];

describe('quality remap', () => {
	it('maps by identity, and drops what is gone or out of range', () => {
		const [a, b, c] = LEVELS;
		expect(levelMapping([a, b, c], [a, c])).toEqual([0, -1, 1]);
		expect(remapIndex(null, [0])).toBeNull();
		expect(remapIndex(5, [0])).toBeNull();
		expect(remapIndex(1, [0, -1])).toBeNull();
		expect([
			...remapKeys(
				new Map([
					[0, 'x'],
					[1, 'y'],
					[2, 'z']
				]),
				[-1, 0, 1]
			)
		]).toEqual([
			[0, 'y'],
			[1, 'z']
		]);
	});

	it('a held level removed re-seats below it, or the nearest loadable level to that', () => {
		const left = [LEVELS[0], LEVELS[1]];
		const base = {
			ladder: [0, 1, 2],
			ceiling: 2,
			current: 2,
			engaged: true,
			pinLevel: null,
			unavailable: new Set<number>()
		};
		const removeTop = [0, 1, -1];
		// The nearest level below that is left.
		expect(remapHold(base, removeTop, left, { kind: 'best' }).current).toBe(1);
		// That level left unavailable: the nearest loadable below it.
		const below = remapHold({ ...base, unavailable: new Set([1]) }, removeTop, left, {
			kind: 'best'
		});
		expect(below.current).toBe(0);
		// Nothing loadable at all: the seat stands, for the engine to settle.
		const none = remapHold({ ...base, unavailable: new Set([0, 1]) }, removeTop, left, {
			kind: 'best'
		});
		expect(none.current).toBe(1);
	});

	it('a pin re-seated below holds; one re-seated above, or released, is let go', () => {
		const pinned = {
			ladder: [0, 1, 2],
			ceiling: 2,
			current: 0,
			engaged: true,
			pinLevel: 0,
			unavailable: new Set<number>()
		};
		const removeBottom = [-1, 0, 1];
		const above = remapHold(pinned, removeBottom, [LEVELS[1], LEVELS[2]], { kind: 'best' });
		expect(above.current).toBe(0);
		expect(above.pinLevel).toBeNull();
		const belowKept = remapHold(
			{ ...pinned, current: 1, pinLevel: 1 },
			[0, -1, 1],
			[LEVELS[0], LEVELS[2]],
			{
				kind: 'best'
			}
		);
		expect(belowKept.current).toBe(0);
		expect(belowKept.pinLevel).toBe(0);
		const one = remapHold({ ...pinned, current: 1, pinLevel: 1 }, [-1, -1, 0], [LEVELS[2]], {
			kind: 'best'
		});
		expect(one.engaged).toBe(false);
		expect(one.pinLevel).toBeNull();
	});

	it('levels all replaced, as a content-steering switch does, seat the hold at the setting and let the pin go', () => {
		const state = {
			ladder: [0, 1, 2],
			ceiling: 2,
			current: 1,
			engaged: true,
			pinLevel: 1,
			unavailable: new Set<number>([0])
		};
		const fresh = [{ ...LEVELS[0] }, { ...LEVELS[1] }, { ...LEVELS[2] }];
		const next = remapHold(state, [-1, -1, -1], fresh, { kind: 'best' });
		expect(next.current).toBe(2);
		expect(next.unavailable.size).toBe(0);
		// The setting's level may be above the pin: it is let go.
		expect(next.pinLevel).toBeNull();
		// A short mapping reads as every level gone, the same way.
		expect(remapHold(state, [], fresh, { kind: 'worst' }).current).toBe(0);
	});
});
