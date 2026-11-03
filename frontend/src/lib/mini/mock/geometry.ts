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

export function placeMini(input: PlaceInput): Placement {
	void input;
	return { size: MINI_DEFAULT_SIZE, position: null, displayId: 0 };
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
	return input.start;
}

export function moveWithin(start: Rect, dx: number, dy: number, area: Rect): Rect {
	void dx;
	void dy;
	void area;
	return start;
}
