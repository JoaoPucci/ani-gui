import { describe, expect, it } from 'vitest';
import { windowControlsSide } from './controls';

describe('windowControlsSide', () => {
	it('keeps controls on the left only on macOS (traffic-light convention)', () => {
		expect(windowControlsSide('darwin')).toBe('left');
	});

	it('puts controls on the right on Linux and Windows', () => {
		expect(windowControlsSide('linux')).toBe('right');
		expect(windowControlsSide('win32')).toBe('right');
	});

	it('defaults to the right for an unknown or missing platform', () => {
		expect(windowControlsSide(undefined)).toBe('right');
		expect(windowControlsSide('')).toBe('right');
	});
});
