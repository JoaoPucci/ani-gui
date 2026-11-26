import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	PAD,
	catchClip,
	dimClip,
	litArea,
	overlaps,
	passHoles,
	rectPath,
	roundedPath,
	sameRect
} from './spotlight';

const view = { w: 1280, h: 800, top: 64 };

describe('litArea', () => {
	it('stands PAD around the place', () => {
		expect(litArea({ x: 100, y: 200, w: 300, h: 100 }, view)).toEqual({
			x: 100 - PAD,
			y: 200 - PAD,
			w: 300 + 2 * PAD,
			h: 100 + 2 * PAD
		});
	});

	it('is clipped below the top bar and inside the window', () => {
		expect(litArea({ x: -20, y: 30, w: 1400, h: 900 }, view)).toEqual({
			x: 0,
			y: 64,
			w: 1280,
			h: 800 - 64
		});
	});

	it('is empty, not negative, for a place off screen', () => {
		const lit = litArea({ x: 100, y: -500, w: 100, h: 100 }, view);
		expect(lit.h).toBe(0);
	});

	it('always lies inside the window under the top bar (property)', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -2000, max: 3000 }),
				fc.integer({ min: -2000, max: 3000 }),
				fc.integer({ min: 0, max: 3000 }),
				fc.integer({ min: 0, max: 3000 }),
				(x, y, w, h) => {
					const lit = litArea({ x, y, w, h }, view);
					return (
						lit.x >= 0 &&
						lit.y >= view.top &&
						lit.w >= 0 &&
						lit.h >= 0 &&
						lit.x + lit.w <= view.w &&
						lit.y + lit.h <= Math.max(view.h, view.top)
					);
				}
			)
		);
	});
});

describe('passHoles', () => {
	it('keeps the window buttons with a size and away from the lit place', () => {
		const lit = { x: 0, y: 0, w: 100, h: 100 };
		const keep = { x: 1200, y: 0, w: 40, h: 32 };
		expect(
			passHoles([keep, { x: 50, y: 10, w: 40, h: 32 }, { x: 900, y: 0, w: 0, h: 32 }], lit)
		).toEqual([keep]);
	});

	it('keeps every sized button when nothing is lit', () => {
		const b = { x: 1, y: 1, w: 2, h: 2 };
		expect(passHoles([b], null)).toEqual([b]);
	});
});

describe('rect helpers', () => {
	it('compares and overlaps', () => {
		const a = { x: 0, y: 0, w: 10, h: 10 };
		expect(sameRect(a, { ...a })).toBe(true);
		expect(sameRect(a, null)).toBe(false);
		expect(sameRect(null, null)).toBe(true);
		expect(overlaps(a, { x: 5, y: 5, w: 10, h: 10 })).toBe(true);
		expect(overlaps(a, { x: 10, y: 0, w: 10, h: 10 })).toBe(false);
	});
});

describe('paths', () => {
	it('writes every rounded rectangle with the same commands, so a cut-out can glide', () => {
		const shape = (p: string) => p.replace(/[-\d.]+/g, '#');
		expect(shape(roundedPath({ x: 1, y: 2, w: 300, h: 40 }, 8))).toBe(
			shape(roundedPath({ x: 900, y: 500, w: 0, h: 0 }, 8))
		);
	});

	it('dims the whole window with one even-odd cut-out at the lit place', () => {
		const clip = dimClip({ x: 10, y: 20, w: 100, h: 50 }, view);
		expect(clip.startsWith('path(evenodd, "M0 0H1280V800H0Z')).toBe(true);
		expect(clip).toContain('M18 20H102');
	});

	it('dims the whole window when nothing is lit', () => {
		expect(dimClip(null, view)).toContain('M640 400H640');
	});

	it('cuts the catcher at the place and at each window button, not at the lit margin', () => {
		const place = { x: 10, y: 20, w: 100, h: 50 };
		const button = { x: 1200, y: 0, w: 40, h: 32 };
		expect(catchClip(place, [button], view)).toBe(
			`path(evenodd, "M0 0H1280V800H0Z${rectPath(place)}${rectPath(button)}")`
		);
		expect(catchClip(null, [], view)).toBe('path(evenodd, "M0 0H1280V800H0Z")');
	});
});
