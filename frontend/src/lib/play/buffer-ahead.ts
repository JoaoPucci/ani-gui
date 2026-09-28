/**
 * How much playable media the player has in hand: the run from the
 * playhead to the end of the buffered range it sits in, across the
 * small gaps the engine steps over on its own. Zero when the playhead
 * sits in no buffered range.
 *
 * A network failure with minutes of this in hand is held rather than
 * recovered from — the user is watching what is already here, and the
 * host that refused a request is best asked again a little later, not
 * asked for the episode from its start. One with none in hand is what
 * the stall recovery is for.
 */

/** The shape of `HTMLMediaElement.buffered`, as much of it as is read. */
export interface BufferedRanges {
	readonly length: number;
	start(index: number): number;
	end(index: number): number;
}

/** Two buffered ranges this close are one run: the engine plays across
 *  a gap of this size without loading anything. */
export const BUFFER_GAP_TOLERANCE_S = 0.5;

export function bufferAheadSeconds(buffered: BufferedRanges, currentTime: number): number {
	const ranges: [number, number][] = [];
	for (let i = 0; i < buffered.length; i++) ranges.push([buffered.start(i), buffered.end(i)]);
	ranges.sort((a, b) => a[0] - b[0]);
	let end: number | null = null;
	for (const [start, stop] of ranges) {
		if (end === null) {
			if (start - BUFFER_GAP_TOLERANCE_S <= currentTime && currentTime <= stop) end = stop;
		} else if (start - end <= BUFFER_GAP_TOLERANCE_S) {
			end = Math.max(end, stop);
		} else {
			break;
		}
	}
	return end === null ? 0 : Math.max(0, end - currentTime);
}
