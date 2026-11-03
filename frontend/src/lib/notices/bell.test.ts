import { describe, expect, it } from 'vitest';
import { countLabel, shouldRing } from './bell';

describe('the bell', () => {
	it('rings when its count goes from nothing to something', () => {
		expect(shouldRing(0, 1, false)).toBe(true);
		expect(shouldRing(0, 4, false)).toBe(true);
	});

	it('does not ring again while the count only changes', () => {
		expect(shouldRing(1, 2, false)).toBe(false);
		expect(shouldRing(3, 0, false)).toBe(false);
		expect(shouldRing(0, 0, false)).toBe(false);
	});

	it('stays still under reduced motion', () => {
		expect(shouldRing(0, 1, true)).toBe(false);
	});

	it('draws at most two digits', () => {
		expect(countLabel(7)).toBe('7');
		expect(countLabel(99)).toBe('99');
		expect(countLabel(100)).toBe('99+');
	});
});
