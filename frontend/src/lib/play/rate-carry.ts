/**
 * The playback speed a switch to another show's player carries over
 * ($lib/play/switch-show.ts). A switch to another show mounts a new
 * page, whose stream would start at normal speed. The switch leaves the
 * speed here. The next player of that show sees it as its stream
 * attaches and takes it once the metadata loads, when it plays: a
 * stream attached again before that still finds it. A player of any
 * other show drops it. Normal speed is not carried.
 */

let pending: { showId: string; rate: number } | null = null;

/** Hand `rate` to the next player of `showId`. */
export function carryRate(showId: string, rate: number): void {
	pending = Number.isFinite(rate) && rate > 0 && rate !== 1 ? { showId, rate } : null;
}

/** Whether a speed is carried to `showId`, without taking it. */
export function rateFor(showId: string): boolean {
	return pending !== null && pending.showId === showId;
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
