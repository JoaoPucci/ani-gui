/**
 * The spotlight's geometry: the lit area around a place, the dim's
 * cut-out, and the click-catcher's holes. Rendering: viewport pixels
 * from getBoundingClientRect in, CSS `clip-path` paths out.
 *
 * The dim is a layer cut out with `path(evenodd, …)`, not a wide
 * box-shadow: a Windows contrast theme drops box-shadows, and the
 * spotlight would vanish there. The click-catcher is a second layer
 * cut out at the place itself and at the window's own buttons, so a
 * press there reaches what is under it, and a press anywhere else stops
 * at the catcher.
 */
import type { Rect } from './place';

/** The lit area stands this far around its place, so the cut-out and
 *  its outline never touch the place's own text or borders. */
export const PAD = 8;
/** The card radius (`--radius-card`): a path cannot read a variable. */
export const RADIUS = 8;

export interface View {
	w: number;
	h: number;
	/** The top bar's bottom edge: nothing is lit above it. */
	top: number;
}

/** The lit area: the place with PAD around it, clipped to the window
 *  below the top bar. */
export function litArea(place: Rect, view: View): Rect {
	const x = Math.min(Math.max(0, place.x - PAD), view.w);
	const y = Math.min(Math.max(view.top, place.y - PAD), Math.max(view.h, view.top));
	const r = Math.min(view.w, place.x + place.w + PAD);
	const b = Math.min(view.h, place.y + place.h + PAD);
	return { x, y, w: Math.max(0, r - x), h: Math.max(0, b - y) };
}

export const sameRect = (a: Rect | null, b: Rect | null): boolean =>
	a === b || (!!a && !!b && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h);

export const overlaps = (a: Rect, b: Rect): boolean =>
	a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** The window's own buttons a press must still reach under the dim:
 *  those with a size, and none over the lit place (an even-odd path
 *  would fill two overlapping holes in again). */
export function passHoles(buttons: Rect[], lit: Rect | null): Rect[] {
	return buttons.filter((b) => b.w > 0 && b.h > 0 && !(lit && overlaps(b, lit)));
}

const n = (v: number) => Math.round(v * 100) / 100;

/** A rounded rectangle, always written with the same commands whatever
 *  its size, so `clip-path` can interpolate one cut-out into the next. */
export function roundedPath(r: Rect, radius: number): string {
	const k = Math.max(0, Math.min(radius, r.w / 2, r.h / 2));
	const { x, y, w, h } = r;
	return (
		`M${n(x + k)} ${n(y)}H${n(x + w - k)}A${n(k)} ${n(k)} 0 0 1 ${n(x + w)} ${n(y + k)}` +
		`V${n(y + h - k)}A${n(k)} ${n(k)} 0 0 1 ${n(x + w - k)} ${n(y + h)}` +
		`H${n(x + k)}A${n(k)} ${n(k)} 0 0 1 ${n(x)} ${n(y + h - k)}` +
		`V${n(y + k)}A${n(k)} ${n(k)} 0 0 1 ${n(x + k)} ${n(y)}Z`
	);
}

export const rectPath = (r: Rect): string =>
	`M${n(r.x)} ${n(r.y)}H${n(r.x + r.w)}V${n(r.y + r.h)}H${n(r.x)}Z`;

const windowPath = (view: { w: number; h: number }) => `M0 0H${n(view.w)}V${n(view.h)}H0Z`;

/** The dim: the whole window, cut out at `lit` (none: a zero hole at
 *  the window's centre, so the dim is whole and can still interpolate). */
export function dimClip(lit: Rect | null, view: { w: number; h: number }): string {
	const hole = lit ?? { x: view.w / 2, y: view.h / 2, w: 0, h: 0 };
	return `path(evenodd, "${windowPath(view)}${roundedPath(hole, RADIUS)}")`;
}

/** The catcher: the whole window, cut out at the place itself and at
 *  each pass hole. The lit margin around the place still catches. */
export function catchClip(
	place: Rect | null,
	holes: Rect[],
	view: { w: number; h: number }
): string {
	const cuts = [...(place ? [place] : []), ...holes].map(rectPath).join('');
	return `path(evenodd, "${windowPath(view)}${cuts}")`;
}
