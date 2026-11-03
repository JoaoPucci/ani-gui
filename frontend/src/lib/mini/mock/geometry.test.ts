import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	MINI_DEFAULT_SIZE,
	MINI_EDGE_INSET,
	MINI_MIN_SIZE,
	moveWithin,
	placeMini,
	resizeAtRatio,
	type MiniDisplay,
	type Rect
} from './geometry';

const RATIO = 16 / 9;
const laptop: MiniDisplay = { id: 1, workArea: { x: 0, y: 32, width: 1920, height: 1048 } };
const side: MiniDisplay = { id: 2, workArea: { x: 1920, y: 0, width: 2560, height: 1440 } };

const inside = (r: Rect, area: Rect) =>
	r.x >= area.x &&
	r.y >= area.y &&
	r.x + r.width <= area.x + area.width &&
	r.y + r.height <= area.y + area.height;

describe('where the mini window opens', () => {
	it('opens at the default size in the main display bottom-right corner, 24 px in', () => {
		const p = placeMini({
			remembered: null,
			displays: [laptop, side],
			mainDisplayId: 1,
			positionAllowed: true,
			ratio: RATIO
		});
		expect(p.size).toEqual(MINI_DEFAULT_SIZE);
		expect(p.displayId).toBe(1);
		expect(p.position).toEqual({
			x: 1920 - MINI_EDGE_INSET - 480,
			y: 32 + 1048 - MINI_EDGE_INSET - 270
		});
	});

	it('reopens where it was left, on the display it was on', () => {
		const bounds = { x: 2000, y: 100, width: 640, height: 360 };
		const p = placeMini({
			remembered: { bounds, displayId: 2 },
			displays: [laptop, side],
			mainDisplayId: 1,
			positionAllowed: true,
			ratio: RATIO
		});
		expect(p).toEqual({
			size: { width: 640, height: 360 },
			position: { x: 2000, y: 100 },
			displayId: 2
		});
	});

	it('falls back to the corner when its display is gone, keeping its size', () => {
		const p = placeMini({
			remembered: { bounds: { x: 2000, y: 100, width: 640, height: 360 }, displayId: 2 },
			displays: [laptop],
			mainDisplayId: 1,
			positionAllowed: true,
			ratio: RATIO
		});
		expect(p.displayId).toBe(1);
		expect(p.size).toEqual({ width: 640, height: 360 });
		expect(p.position).toEqual({ x: 1920 - 24 - 640, y: 32 + 1048 - 24 - 360 });
	});

	it('falls back to the corner when the place no longer fits its work area', () => {
		const p = placeMini({
			remembered: { bounds: { x: 1700, y: 900, width: 480, height: 270 }, displayId: 1 },
			displays: [laptop],
			mainDisplayId: 1,
			positionAllowed: true,
			ratio: RATIO
		});
		expect(p.position).toEqual({ x: 1920 - 24 - 480, y: 32 + 1048 - 24 - 270 });
	});

	it('keeps only the size on Wayland, where the compositor places windows', () => {
		const p = placeMini({
			remembered: { bounds: { x: 0, y: 0, width: 640, height: 360 }, displayId: 1 },
			displays: [laptop],
			mainDisplayId: 1,
			positionAllowed: false,
			ratio: RATIO
		});
		expect(p.position).toBeNull();
		expect(p.size).toEqual({ width: 640, height: 360 });
	});

	it('takes the stream ratio, and uses the first display when the main one is unknown', () => {
		const p = placeMini({
			remembered: null,
			displays: [side],
			mainDisplayId: 9,
			positionAllowed: true,
			ratio: 4 / 3
		});
		expect(p.displayId).toBe(2);
		expect(p.size).toEqual({ width: 480, height: 360 });
	});

	it('always opens at the ratio, at least the minimum, inside a work area', () => {
		const area = fc.record({
			x: fc.integer({ min: -4000, max: 4000 }),
			y: fc.integer({ min: -2000, max: 2000 }),
			width: fc.integer({ min: 800, max: 5000 }),
			height: fc.integer({ min: 600, max: 3000 })
		});
		fc.assert(
			fc.property(
				fc.array(area, { minLength: 1, maxLength: 3 }),
				fc.option(
					fc.record({
						bounds: fc.record({
							x: fc.integer({ min: -5000, max: 9000 }),
							y: fc.integer({ min: -3000, max: 5000 }),
							width: fc.integer({ min: 1, max: 6000 }),
							height: fc.integer({ min: 1, max: 4000 })
						}),
						displayId: fc.integer({ min: 0, max: 4 })
					}),
					{ nil: null }
				),
				fc.integer({ min: 0, max: 4 }),
				fc.boolean(),
				fc.constantFrom(16 / 9, 4 / 3, 2.39, 1.85),
				(areas, remembered, mainDisplayId, positionAllowed, ratio) => {
					const displays = areas.map((workArea, i) => ({ id: i, workArea }));
					const p = placeMini({ remembered, displays, mainDisplayId, positionAllowed, ratio });
					expect(p.size.width).toBeGreaterThanOrEqual(MINI_MIN_SIZE.width);
					expect(Math.abs(p.size.width / ratio - p.size.height)).toBeLessThanOrEqual(1);
					const display = displays.find((d) => d.id === p.displayId);
					expect(display).toBeDefined();
					if (positionAllowed) {
						expect(p.position).not.toBeNull();
						expect(inside({ ...p.position!, ...p.size }, display!.workArea)).toBe(true);
					} else {
						expect(p.position).toBeNull();
					}
				}
			)
		);
	});
});

describe('resizing from the corner grip', () => {
	const start = { x: 100, y: 100, width: 480, height: 270 };
	const limits = { ratio: RATIO, min: MINI_MIN_SIZE, max: { width: 1280, height: 720 } };

	it('grows and shrinks at the ratio, from the top-left corner', () => {
		expect(resizeAtRatio({ start, dx: 160, dy: 0, ...limits })).toEqual({
			x: 100,
			y: 100,
			width: 640,
			height: 360
		});
		expect(resizeAtRatio({ start, dx: 0, dy: 90, ...limits })).toEqual({
			x: 100,
			y: 100,
			width: 640,
			height: 360
		});
	});

	it('stops at the minimum and the maximum', () => {
		expect(resizeAtRatio({ start, dx: -900, dy: -900, ...limits })).toMatchObject(MINI_MIN_SIZE);
		expect(resizeAtRatio({ start, dx: 5000, dy: 0, ...limits })).toMatchObject({
			width: 1280,
			height: 720
		});
	});

	it('keeps the corner, the ratio and the limits for any drag', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -3000, max: 3000 }),
				fc.integer({ min: -3000, max: 3000 }),
				(dx, dy) => {
					const r = resizeAtRatio({ start, dx, dy, ...limits });
					expect(r.x).toBe(start.x);
					expect(r.y).toBe(start.y);
					expect(r.width).toBeGreaterThanOrEqual(MINI_MIN_SIZE.width);
					expect(r.width).toBeLessThanOrEqual(1280);
					expect(Math.abs(r.width / RATIO - r.height)).toBeLessThanOrEqual(1);
				}
			)
		);
	});
});

describe('dragging by the strip', () => {
	const area = { x: 0, y: 0, width: 1280, height: 720 };

	it('moves by the drag and stays inside the area', () => {
		const start = { x: 100, y: 100, width: 480, height: 270 };
		expect(moveWithin(start, 50, -20, area)).toEqual({ ...start, x: 150, y: 80 });
		expect(moveWithin(start, -500, -500, area)).toEqual({ ...start, x: 0, y: 0 });
		expect(moveWithin(start, 5000, 5000, area)).toEqual({ ...start, x: 800, y: 450 });
	});

	it('never leaves the area for any drag', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -5000, max: 5000 }),
				fc.integer({ min: -5000, max: 5000 }),
				(dx, dy) => {
					const r = moveWithin({ x: 300, y: 200, width: 480, height: 270 }, dx, dy, area);
					expect(inside(r, area)).toBe(true);
					expect(r.width).toBe(480);
				}
			)
		);
	});
});
