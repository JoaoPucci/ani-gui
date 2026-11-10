// The hero's scroll parallax on Home and on a show's page.
import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { heroParallaxTransform } from './parallax';

const offsetOf = (transform: string) =>
	Number(/translate3d\(0, (-?[\d.]+)px, 0\)/.exec(transform)?.[1]);

describe('heroParallaxTransform', () => {
	it('drifts a quarter of the scroll, up to 80px, while motion is on', () => {
		fc.assert(
			fc.property(fc.integer({ min: 0, max: 10_000 }), fc.boolean(), (y, isCover) => {
				expect(offsetOf(heroParallaxTransform(y, isCover, false))).toBeCloseTo(
					Math.min(y * 0.25, 80)
				);
			})
		);
	});

	it('holds still under reduced motion, however far the page scrolls', () => {
		fc.assert(
			fc.property(fc.integer({ min: 0, max: 10_000 }), fc.boolean(), (y, isCover) => {
				expect(heroParallaxTransform(y, isCover, true)).toBe(
					heroParallaxTransform(0, isCover, false)
				);
			})
		);
	});

	it('keeps the bleed scale, which does not move', () => {
		expect(heroParallaxTransform(400, true, true)).toBe('translate3d(0, 0px, 0) scale(1.02)');
		expect(heroParallaxTransform(400, false, true)).toBe('translate3d(0, 0px, 0) scale(1.15)');
		expect(heroParallaxTransform(40, true, false)).toBe('translate3d(0, 10px, 0) scale(1.02)');
	});
});
