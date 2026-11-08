/**
 * The playback speed a switch to another show's player carries over
 * ($lib/play/switch-show.ts). An episode switch keeps the page, and its
 * speed with it; a switch to another show mounts a new page, which
 * would start at normal speed. The switch leaves the speed here, and
 * the next player of that show takes it once as its stream attaches.
 * A player of any other show drops it. Normal speed is not carried.
 */

let pending: { showId: string; rate: number } | null = null;

/** Hand `rate` to the next player of `showId`. */
export function carryRate(showId: string, rate: number): void {
	pending = Number.isFinite(rate) && rate > 0 && rate !== 1 ? { showId, rate } : null;
}

/** The speed carried to `showId`, once; null when none. Taking clears
 *  it either way. */
export function takeRate(showId: string): number | null {
	const p = pending;
	pending = null;
	return p !== null && p.showId === showId ? p.rate : null;
}

/** Test seam: forget any carried speed. */
export function __resetRateForTests(): void {
	pending = null;
}
