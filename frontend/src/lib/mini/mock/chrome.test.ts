import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { MINI_FADE_MS, MINI_IDLE_HIDE_MS, miniChromeShown, miniFadeMs } from './chrome';

const rest = {
	pointerActive: false,
	focusWithin: false,
	paused: false,
	timelineHover: false,
	menuOpen: false,
	cardShown: false
};

describe('the mini window chrome', () => {
	it('is hidden at rest while the episode plays', () => {
		expect(miniChromeShown(rest)).toBe(false);
	});

	it('shows for each reason to stay, alone', () => {
		for (const key of Object.keys(rest) as (keyof typeof rest)[]) {
			expect(miniChromeShown({ ...rest, [key]: true })).toBe(true);
		}
	});

	it('hides only when no reason to stay holds', () => {
		fc.assert(
			fc.property(
				fc.record({
					pointerActive: fc.boolean(),
					focusWithin: fc.boolean(),
					paused: fc.boolean(),
					timelineHover: fc.boolean(),
					menuOpen: fc.boolean(),
					cardShown: fc.boolean()
				}),
				(state) => {
					expect(miniChromeShown(state)).toBe(Object.values(state).some(Boolean));
				}
			)
		);
	});

	it('leaves after the main player fullscreen idle time and fades briefly', () => {
		expect(MINI_IDLE_HIDE_MS).toBe(2500);
		expect(miniFadeMs(false)).toBe(MINI_FADE_MS);
		expect(MINI_FADE_MS).toBe(140);
	});

	it('appears and disappears at once under reduced motion', () => {
		expect(miniFadeMs(true)).toBe(0);
	});
});
