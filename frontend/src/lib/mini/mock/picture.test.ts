import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { PICTURE_CLICK_DELAY_MS, decidePictureClick, seekFraction } from './picture';

describe('a click on the picture', () => {
	it('only closes an open menu', () => {
		expect(decidePictureClick(true)).toBe('closeMenus');
	});

	it('otherwise plays or pauses after the double-click window', () => {
		expect(decidePictureClick(false)).toBe('toggleLater');
		expect(PICTURE_CLICK_DELAY_MS).toBe(220);
	});
});

describe('where along the timeline a pointer is', () => {
	it('reads from the start, or from the end right to left', () => {
		expect(seekFraction(150, 100, 200, false)).toBe(0.25);
		expect(seekFraction(150, 100, 200, true)).toBe(0.75);
	});

	it('stays between 0 and 1, and is 0 on a timeline with no width', () => {
		expect(seekFraction(0, 100, 200, false)).toBe(0);
		expect(seekFraction(900, 100, 200, false)).toBe(1);
		expect(seekFraction(150, 100, 0, false)).toBe(0);
	});

	it('mirrors exactly between the two directions', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -500, max: 1500 }),
				fc.integer({ min: 0, max: 400 }),
				fc.integer({ min: 1, max: 1000 }),
				(x, left, width) => {
					const ltr = seekFraction(x, left, width, false);
					expect(ltr).toBeGreaterThanOrEqual(0);
					expect(ltr).toBeLessThanOrEqual(1);
					expect(seekFraction(x, left, width, true)).toBeCloseTo(1 - ltr, 10);
				}
			)
		);
	});
});
