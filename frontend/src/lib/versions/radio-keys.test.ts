import { describe, expect, it } from 'vitest';
import { radioKeyTarget } from './radio-keys';

describe('radioKeyTarget', () => {
	it('moves forward on the down and inline-forward arrows, wrapping at the end', () => {
		expect(radioKeyTarget('ArrowDown', 0, 2)).toBe(1);
		expect(radioKeyTarget('ArrowRight', 0, 2)).toBe(1);
		expect(radioKeyTarget('ArrowRight', 1, 2)).toBe(0);
	});

	it('moves back on the up and inline-backward arrows, wrapping at the start', () => {
		expect(radioKeyTarget('ArrowUp', 1, 2)).toBe(0);
		expect(radioKeyTarget('ArrowLeft', 1, 2)).toBe(0);
		expect(radioKeyTarget('ArrowLeft', 0, 3)).toBe(2);
	});

	it('swaps the horizontal arrows in a right-to-left layout, and only those', () => {
		expect(radioKeyTarget('ArrowLeft', 0, 3, true)).toBe(1);
		expect(radioKeyTarget('ArrowRight', 0, 3, true)).toBe(2);
		expect(radioKeyTarget('ArrowDown', 0, 3, true)).toBe(1);
	});

	it('jumps to the ends on Home and End', () => {
		expect(radioKeyTarget('Home', 2, 3)).toBe(0);
		expect(radioKeyTarget('End', 0, 3)).toBe(2);
	});

	it('answers nothing for any other key, or an empty group', () => {
		expect(radioKeyTarget('Enter', 0, 2)).toBeNull();
		expect(radioKeyTarget('a', 0, 2)).toBeNull();
		expect(radioKeyTarget('ArrowDown', 0, 0)).toBeNull();
	});
});
