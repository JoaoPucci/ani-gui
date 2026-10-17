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

	it('a held level removed moves to the ceiling, or to the nearest loadable level to it', () => {
		const left = [LEVELS[0], LEVELS[1]];
		const base = {
			ladder: [0, 1, 2],
			ceiling: 2,
			current: 2,
			engaged: true,
			unavailable: new Set<number>()
		};
		// Ceiling and held level both gone: the setting's level among those left.
		expect(remapHold(base, [0, 1, -1], left, { kind: 'best' }).current).toBe(1);
		// That level left unavailable: the nearest below it.
		const below = remapHold({ ...base, unavailable: new Set([1]) }, [0, 1, -1], left, {
			kind: 'best'
		});
		expect(below.current).toBe(0);
		// Nothing below either: the nearest above.
		const above = remapHold(
			{ ...base, ceiling: 0, current: 2, unavailable: new Set([0]) },
			[0, 1, -1],
			left,
			{ kind: 'worst' }
		);
		expect(above.current).toBe(1);
		// Nothing loadable at all: the ceiling, for the engine to settle.
		const none = remapHold(
			{ ...base, ceiling: 0, current: 2, unavailable: new Set([0, 1]) },
			[0, 1, -1],
			left,
			{ kind: 'worst' }
		);
		expect(none.current).toBe(0);
	});
});
