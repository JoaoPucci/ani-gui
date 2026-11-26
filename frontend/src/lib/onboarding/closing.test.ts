import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { closing } from './closing';
import { PRESS_GRACE_MS } from './gate';

describe('closing', () => {
	it('every way a person closes a tip counts it seen', () => {
		for (const route of ['got-it', 'escape', 'outside', 'place'] as const) {
			expect(closing(route, 0, 10_000)?.seen, route).toBe(true);
		}
	});

	it('a tip that yields stays pending, and leaves focus where the page put it', () => {
		expect(closing('yield', 0, 10_000)).toEqual({ seen: false, focus: 'leave' });
	});

	it('sends focus to the place after Got it, back after Escape or a press outside, and leaves a press on the place alone', () => {
		expect(closing('got-it', 0, 1)?.focus).toBe('place');
		expect(closing('escape', 0, 1)?.focus).toBe('back');
		expect(closing('outside', 0, 10_000)?.focus).toBe('back');
		expect(closing('place', 0, 10_000)?.focus).toBe('none');
	});

	it('a press outside within the grace does nothing, and after it closes (property)', () => {
		fc.assert(
			fc.property(fc.integer({ min: 0, max: 1e7 }), fc.integer({ min: 0, max: 5000 }), (at, d) => {
				const c = closing('outside', at, at + d);
				return d < PRESS_GRACE_MS ? c === null : c?.seen === true;
			})
		);
	});
	it('a press on the place within the grace was aimed before the tip showed: the press works, the tip stays', () => {
		expect(closing('place', 0, PRESS_GRACE_MS - 1)).toBeNull();
		expect(closing('place', 0, PRESS_GRACE_MS)?.seen).toBe(true);
	});
});
