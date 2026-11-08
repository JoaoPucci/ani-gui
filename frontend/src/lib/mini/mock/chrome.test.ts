import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	MINI_FADE_MS,
	MINI_IDLE_HIDE_MS,
	miniChromeShown,
	miniFadeMs,
	miniGripShown,
	miniStripShown
} from './chrome';

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

describe('the strip through a stall', () => {
	it('stays up while the stream stalls, and follows the chrome otherwise', () => {
		expect(miniStripShown(false, true)).toBe(true);
		expect(miniStripShown(false, false)).toBe(false);
		expect(miniStripShown(true, false)).toBe(true);
	});
});

describe('the resize grip', () => {
	it('draws only on Wayland, with the chrome or always', () => {
		expect(miniGripShown({ platform: 'wayland', grip: 'chrome', chromeShown: true })).toBe(true);
		expect(miniGripShown({ platform: 'wayland', grip: 'chrome', chromeShown: false })).toBe(false);
		expect(miniGripShown({ platform: 'wayland', grip: 'always', chromeShown: false })).toBe(true);
	});

	it('never draws where the platform has resize edges', () => {
		fc.assert(
			fc.property(
				fc.constantFrom('windows' as const, 'x11' as const),
				fc.constantFrom('chrome' as const, 'always' as const),
				fc.boolean(),
				(platform, grip, chromeShown) => {
					expect(miniGripShown({ platform, grip, chromeShown })).toBe(false);
				}
			)
		);
	});
});
