import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { MINI_COMPACT_BELOW, miniControlSet } from './fit';

describe('the controls at a width', () => {
	it('draws every control at the default size in the bar layout', () => {
		expect(miniControlSet(480, 'bar')).toEqual({
			compact: false,
			episodeSteps: 'buttons',
			clusterFlanks: null,
			time: 'both',
			mute: true
		});
	});

	it('moves previous and next into the timeline hover at the minimum, in the bar layout', () => {
		expect(miniControlSet(320, 'bar')).toEqual({
			compact: true,
			episodeSteps: 'timeline',
			clusterFlanks: null,
			time: 'current',
			mute: false
		});
	});

	it('flanks the centre cluster with ten-second skips, and episode steps by the timeline', () => {
		expect(miniControlSet(480, 'centre')).toEqual({
			compact: false,
			episodeSteps: 'buttons',
			clusterFlanks: 'skip',
			time: 'both',
			mute: true
		});
		expect(miniControlSet(320, 'centre')).toMatchObject({
			compact: true,
			episodeSteps: 'timeline',
			clusterFlanks: 'skip',
			time: 'current'
		});
	});

	it('switches at one width for both layouts', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 320, max: 2000 }),
				fc.constantFrom('bar' as const, 'centre' as const),
				(width, layout) => {
					expect(miniControlSet(width, layout).compact).toBe(width < MINI_COMPACT_BELOW);
				}
			)
		);
	});
});
