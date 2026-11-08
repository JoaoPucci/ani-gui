import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { MINI_COMPACT_BELOW, cueLifts, cueSizePx, miniControlSet } from './fit';

describe('the controls at a width', () => {
	it('draws every control at the default size in the bar layout', () => {
		expect(miniControlSet(480, 'bar')).toEqual({
			compact: false,
			episodeSteps: 'buttons',
			clusterFlanks: null,
			time: 'both',
			mute: true,
			cueLift: 56
		});
	});

	it('hides previous, next and the volume at the minimum, rather than hover them over the timeline', () => {
		expect(miniControlSet(320, 'bar')).toEqual({
			compact: true,
			episodeSteps: 'hidden',
			clusterFlanks: null,
			time: 'current',
			mute: false,
			cueLift: 36
		});
	});

	it('flanks the centre cluster with ten-second skips and draws no episode buttons', () => {
		expect(miniControlSet(480, 'centre')).toEqual({
			compact: false,
			episodeSteps: 'hidden',
			clusterFlanks: 'skip',
			time: 'both',
			mute: true,
			cueLift: 32
		});
		expect(miniControlSet(320, 'centre')).toMatchObject({
			compact: true,
			episodeSteps: 'hidden',
			clusterFlanks: 'skip',
			time: 'current',
			cueLift: 24
		});
	});

	it('switches at one width for both layouts, and lifts cues less when compact', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 320, max: 2000 }),
				fc.constantFrom('bar' as const, 'centre' as const),
				(width, layout) => {
					const set = miniControlSet(width, layout);
					expect(set.compact).toBe(width < MINI_COMPACT_BELOW);
					expect(set.cueLift).toBeLessThanOrEqual(miniControlSet(480, layout).cueLift);
					expect(set.cueLift).toBeGreaterThan(0);
				}
			)
		);
	});
});

describe('which cues rise with the chrome', () => {
	it('lifts the cues the track leaves at the bottom', () => {
		expect(cueLifts('auto', true)).toBe(true);
		expect(cueLifts(-1, true)).toBe(true);
		expect(cueLifts(90, false)).toBe(true);
		expect(cueLifts(50, false)).toBe(true);
	});

	it('leaves a cue placed in the upper half where the track put it', () => {
		expect(cueLifts(0, true)).toBe(false);
		expect(cueLifts(2, true)).toBe(false);
		expect(cueLifts(10, false)).toBe(false);
		expect(cueLifts(49.9, false)).toBe(false);
	});

	it('decides a percentage position by the middle of the picture', () => {
		fc.assert(
			fc.property(fc.double({ min: 0, max: 100, noNaN: true }), (line) => {
				expect(cueLifts(line, false)).toBe(line >= 50);
			})
		);
	});

	it('lifts a line number exactly when it counts from the bottom', () => {
		fc.assert(
			fc.property(fc.integer({ min: -20, max: 20 }), (line) => {
				expect(cueLifts(line, true)).toBe(line < 0);
			})
		);
	});
});

describe('the subtitle size floor', () => {
	it('keeps cues readable at the minimum size, and follows the picture above it', () => {
		expect(cueSizePx(180)).toBe(14);
		expect(cueSizePx(270)).toBe(15);
		expect(cueSizePx(400)).toBe(22);
		expect(cueSizePx(2000)).toBe(30);
	});

	it('stays between 14 and 30 px and never shrinks as the window grows', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 1, max: 4000 }),
				fc.integer({ min: 0, max: 500 }),
				(h, more) => {
					const size = cueSizePx(h);
					expect(size).toBeGreaterThanOrEqual(14);
					expect(size).toBeLessThanOrEqual(30);
					expect(cueSizePx(h + more)).toBeGreaterThanOrEqual(size);
				}
			)
		);
	});
});
