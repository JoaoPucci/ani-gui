/**
 * Where a tip's balloon goes beside the place it explains: below,
 * above, after or before it, whichever fits inside the window's 16px
 * gutter and under the top bar, and a sheet along the bottom of a
 * window too narrow to point from. Rendering: viewport pixels in,
 * viewport pixels out.
 */
export interface Rect {
	x: number;
	y: number;
	w: number;
	h: number;
}
export interface Placement {
	mode: 'anchored' | 'sheet';
	side: 'below' | 'above' | 'end' | 'start';
	x: number;
	y: number;
	width: number;
	/** False when no side had room and the balloon overlaps its place. */
	fits: boolean;
	/** The pointer's offset along the balloon's edge, from its start. */
	arrow: number;
}

const GUTTER = 16;
const GAP = 12;
/** The lit area stands this far around its place (spotlight.ts PAD). */
const LIT_PAD = 8;
/** Narrower than this, the balloon is a sheet along the bottom. */
export const SHEET_BELOW = 560;

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), Math.max(lo, hi));

export function placeBalloon(
	target: Rect,
	size: { w: number; h: number },
	view: { w: number; h: number; top: number }
): Placement {
	// The highest a balloon may sit: under the top bar, with the gutter.
	const minY = view.top + GUTTER;
	if (view.w < SHEET_BELOW) {
		return {
			mode: 'sheet',
			side: 'below',
			fits: true,
			x: GUTTER,
			y: Math.max(minY, view.h - size.h - GUTTER),
			width: view.w - 2 * GUTTER,
			arrow: 0
		};
	}
	const width = Math.min(size.w, view.w - 2 * GUTTER);
	const maxX = view.w - width - GUTTER;
	const maxY = view.h - size.h - GUTTER;
	const midX = target.x + target.w / 2;
	const midY = target.y + target.h / 2;
	const across = (y: number, side: Placement['side'], fits = true): Placement => {
		const x = clamp(midX - width / 2, GUTTER, maxX);
		return { mode: 'anchored', side, fits, x, y, width, arrow: clamp(midX - x, 20, width - 20) };
	};
	const beside = (x: number, side: Placement['side']): Placement => {
		const y = clamp(midY - size.h / 2, minY, maxY);
		return {
			mode: 'anchored',
			side,
			fits: true,
			x,
			y,
			width,
			arrow: clamp(midY - y, 20, size.h - 20)
		};
	};
	const below = target.y + target.h + GAP;
	if (below + size.h <= view.h - GUTTER && below >= minY) return across(below, 'below');
	const above = target.y - GAP - size.h;
	if (above >= minY && above <= maxY) return across(above, 'above');
	const end = target.x + target.w + GAP;
	if (end + width <= view.w - GUTTER) return beside(end, 'end');
	const start = target.x - GAP - width;
	// A place hanging off the window's end still gets a balloon inside it.
	if (start >= GUTTER) return beside(Math.min(start, maxX), 'start');
	// No side has room: over the place, inside the window. The caller
	// scrolls the place into room and asks again.
	return across(clamp(below, minY, maxY), 'below', false);
}

/** Where a place with no room beside it for its balloon is scrolled
 *  to, as its distance from the window's top: under the top bar
 *  (`top`), with the gutter, the balloon, the gap and the lit area's
 *  margin above it, so the balloon fits above. */
export function roomAbove(top: number, size: { h: number }): number {
	return top + GUTTER + size.h + GAP + LIT_PAD;
}
