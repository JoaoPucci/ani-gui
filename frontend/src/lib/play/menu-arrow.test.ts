import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { menuArrow } from './menu-arrow';

describe('menuArrow', () => {
	const at = (over: Partial<Parameters<typeof menuArrow>[1]> = {}) => ({
		rtl: false,
		listOpen: false,
		onOpener: false,
		...over
	});

	it('opens the list from the row that opens it with the arrow pointing into the line', () => {
		expect(menuArrow('ArrowRight', at({ onOpener: true }))).toBe('open');
		expect(menuArrow('ArrowLeft', at({ onOpener: true, rtl: true }))).toBe('open');
	});

	it('goes back from the list with the arrow pointing out of it', () => {
		expect(menuArrow('ArrowLeft', at({ listOpen: true }))).toBe('back');
		expect(menuArrow('ArrowRight', at({ listOpen: true, rtl: true }))).toBe('back');
	});

	it('keeps any other horizontal arrow in the menu', () => {
		expect(menuArrow('ArrowLeft', at({ onOpener: true }))).toBe('stay');
		expect(menuArrow('ArrowRight', at())).toBe('stay');
		expect(menuArrow('ArrowLeft', at())).toBe('stay');
		expect(menuArrow('ArrowRight', at({ listOpen: true }))).toBe('stay');
	});

	it('leaves every other key alone', () => {
		fc.assert(
			fc.property(
				fc.string().filter((k) => k !== 'ArrowLeft' && k !== 'ArrowRight'),
				fc.boolean(),
				fc.boolean(),
				fc.boolean(),
				(key, rtl, listOpen, onOpener) => {
					expect(menuArrow(key, { rtl, listOpen, onOpener })).toBeNull();
				}
			)
		);
	});

	it('answers every horizontal arrow, mirrored under right-to-left', () => {
		fc.assert(
			fc.property(
				fc.constantFrom('ArrowLeft', 'ArrowRight'),
				fc.boolean(),
				fc.boolean(),
				(key, listOpen, onOpener) => {
					const ltr = menuArrow(key, { rtl: false, listOpen, onOpener });
					const mirrored = key === 'ArrowLeft' ? 'ArrowRight' : 'ArrowLeft';
					expect(ltr).not.toBeNull();
					expect(menuArrow(mirrored, { rtl: true, listOpen, onOpener })).toBe(ltr);
				}
			)
		);
	});
});
