import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { placeBalloon, roomAbove } from './place';
import { PAD, litArea } from './spotlight';

const view = { w: 1280, h: 800, top: 0 };
const size = { w: 340, h: 180 };

describe('placeBalloon', () => {
	it('sits below a place near the top, pointing at its middle', () => {
		const p = placeBalloon({ x: 400, y: 100, w: 600, h: 200 }, size, view);
		expect(p).toMatchObject({ mode: 'anchored', side: 'below', y: 312 });
		expect(p.x + p.arrow).toBe(700);
	});
	it('goes above a place near the bottom', () => {
		expect(placeBalloon({ x: 400, y: 560, w: 600, h: 200 }, size, view).side).toBe('above');
	});
	it('goes beside a place as tall as the window', () => {
		expect(placeBalloon({ x: 100, y: 20, w: 400, h: 760 }, size, view).side).toBe('end');
	});
	it('becomes a sheet along the bottom in a narrow window', () => {
		const p = placeBalloon({ x: 10, y: 100, w: 300, h: 100 }, size, { w: 520, h: 700, top: 0 });
		expect(p).toMatchObject({ mode: 'sheet', x: 16, width: 488 });
	});
	it('never goes over the top bar', () => {
		const p = placeBalloon({ x: 400, y: 250, w: 600, h: 400 }, size, { ...view, top: 84 });
		expect(p.side).not.toBe('above');
		expect(p.y).toBeGreaterThanOrEqual(100);
	});
	it('says when the place is too tall for any side, so the page can scroll it into room', () => {
		expect(
			placeBalloon({ x: 200, y: 120, w: 800, h: 420 }, size, { w: 960, h: 600, top: 84 }).fits
		).toBe(false);
		expect(placeBalloon({ x: 400, y: 100, w: 600, h: 200 }, size, view).fits).toBe(true);
	});
	it('always stays inside the window with its gutter (property)', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 560, max: 3000 }),
				fc.integer({ min: 400, max: 2000 }),
				fc.integer({ min: 0, max: 2900 }),
				fc.integer({ min: 0, max: 1900 }),
				fc.integer({ min: 8, max: 800 }),
				fc.integer({ min: 8, max: 800 }),
				(vw, vh, tx, ty, tw, th) => {
					const p = placeBalloon({ x: tx % vw, y: ty % vh, w: tw, h: th }, size, {
						w: vw,
						h: vh,
						top: 0
					});
					return (
						p.x >= 16 &&
						p.x + p.width <= vw - 16 &&
						p.y >= 16 &&
						p.y + size.h <= Math.max(vh - 16, 16 + size.h)
					);
				}
			)
		);
	});
	it('a place scrolled to roomAbove under the top bar gets its balloon above it, fitting', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 0, max: 120 }),
				fc.integer({ min: 80, max: 260 }),
				fc.integer({ min: 200, max: 900 }),
				(top, h, placeH) => {
					const v = { w: 1280, h: 2400, top };
					const balloon = { w: 340, h };
					const y = roomAbove(v.top, balloon);
					const lit = litArea({ x: 300, y, w: 600, h: placeH }, v);
					const p = placeBalloon(lit, balloon, { ...v, h: y + placeH + 40 });
					return p.fits && p.side === 'above' && lit.y - PAD >= 0;
				}
			)
		);
	});
});
