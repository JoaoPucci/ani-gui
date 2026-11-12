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

/** The element side of a carried speed: when one is carried to
 *  `showId`, play it on `video` once the stream's metadata has loaded —
 *  the load resets the rate, a stream attached again before then still
 *  finds it, and the element's default stays as it was, so a later
 *  episode does not fall back to it. The returned function stops
 *  waiting (the source went); null when nothing is carried. */
export function applyCarriedRate(
	video: Pick<HTMLMediaElement, 'addEventListener' | 'removeEventListener' | 'playbackRate'>,
	showId: string
): (() => void) | null {
	if (!rateFor(showId)) return null;
	const play = () => {
		const rate = takeRate(showId);
		if (rate !== null) video.playbackRate = rate;
	};
	video.addEventListener('loadedmetadata', play, { once: true });
	return () => video.removeEventListener('loadedmetadata', play);
}

/** Test seam: forget any carried speed. */
export function __resetRateForTests(): void {
	pending = null;
}
