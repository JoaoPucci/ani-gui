/**
 * The mini window's size and place.
 *
 * `placeMini` is where the window opens: its remembered size
 * everywhere, and its remembered place where the platform lets an app
 * choose one (X11 and Windows, not Wayland, where the compositor
 * places windows). A place on a display that is gone, or that no
 * longer fits inside its work area, falls back to the bottom-right
 * corner of the main window's display, MINI_EDGE_INSET in from the
 * edges. The size keeps the stream's ratio and stays between the
 * minimum and the display's work area.
 *
 * `resizeAtRatio` is a drag of the corner grip, which keeps the
 * top-left corner where it is; `moveWithin` is a drag of the strip,
 * kept inside an area. The mockup draws its window inside the main
 * window, so the "display" there is the page's viewport.
 */

export interface Rect {
	x: number;
	y: number;
	width: number;
	height: number;
}

export interface Size {
	width: number;
	height: number;
}

export interface MiniDisplay {
	id: number;
	workArea: Rect;
}

export interface RememberedBounds {
	bounds: Rect;
	displayId: number;
}

export const MINI_DEFAULT_SIZE: Size = { width: 480, height: 270 };
export const MINI_MIN_SIZE: Size = { width: 320, height: 180 };
export const MINI_EDGE_INSET = 24;

export interface PlaceInput {
	remembered: RememberedBounds | null;
	displays: readonly MiniDisplay[];
	mainDisplayId: number;
	/** False on Wayland: the size applies, the compositor places it. */
	positionAllowed: boolean;
	/** The stream's width over its height. */
	ratio: number;
}

export interface Placement {
	size: Size;
	/** Absent where the platform places the window. */
	position: { x: number; y: number } | null;
	displayId: number;
}

const fits = (r: Rect, area: Rect) =>
	r.x >= area.x &&
	r.y >= area.y &&
	r.x + r.width <= area.x + area.width &&
	r.y + r.height <= area.y + area.height;

/** `width` at `ratio`, at least the minimum width and at most what the
 *  area holds; the height follows, rounded. */
function sizeAt(width: number, ratio: number, area: Rect): Size {
	const most = Math.min(area.width, Math.floor(area.height * ratio));
	const w = Math.round(Math.max(MINI_MIN_SIZE.width, Math.min(width, most)));
	return { width: w, height: Math.round(w / ratio) };
}

export function placeMini(input: PlaceInput): Placement {
	const { remembered, displays, ratio } = input;
	const home = displays.find((d) => d.id === input.mainDisplayId) ?? displays[0];
	const own = remembered ? displays.find((d) => d.id === remembered.displayId) : undefined;
	const display = own ?? home;
	const size = sizeAt(remembered?.bounds.width ?? MINI_DEFAULT_SIZE.width, ratio, display.workArea);
	if (!input.positionAllowed) return { size, position: null, displayId: display.id };
	if (own && remembered) {
		const kept = { x: remembered.bounds.x, y: remembered.bounds.y, ...size };
		if (fits(kept, own.workArea))
			return { size, position: { x: kept.x, y: kept.y }, displayId: own.id };
	}
	const corner = sizeAt(size.width, ratio, home.workArea);
	const area = home.workArea;
	const inset = (room: number, extent: number) =>
		Math.max(0, Math.min(MINI_EDGE_INSET, room - extent));
	return {
		size: corner,
		position: {
			x: area.x + area.width - corner.width - inset(area.width, corner.width),
			y: area.y + area.height - corner.height - inset(area.height, corner.height)
		},
		displayId: home.id
	};
}

export interface ResizeInput {
	start: Rect;
	dx: number;
	dy: number;
	ratio: number;
	min: Size;
	max: Size;
}

export function resizeAtRatio(input: ResizeInput): Rect {
	const { start, ratio, min, max } = input;
	const wanted = Math.max(start.width + input.dx, (start.height + input.dy) * ratio);
	const most = Math.min(max.width, max.height * ratio);
	const least = Math.max(min.width, min.height * ratio);
	const width = Math.round(Math.max(least, Math.min(most, wanted)));
	return { x: start.x, y: start.y, width, height: Math.round(width / ratio) };
}

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

export function moveWithin(start: Rect, dx: number, dy: number, area: Rect): Rect {
	return {
		...start,
		x: clamp(start.x + dx, area.x, area.x + area.width - start.width),
		y: clamp(start.y + dy, area.y, area.y + area.height - start.height)
	};
}
