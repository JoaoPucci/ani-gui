/**
 * Which controls the mini window draws at a width. At the default
 * 480 px and up every control has a place; below MINI_COMPACT_BELOW
 * (the 320 px minimum among them) the bottom row keeps play, the
 * timeline and captions, and previous / next and the volume hide —
 * N, P, M and the arrows still work. Previous / next do not move into
 * the timeline's hover: at about 200 px of timeline, a seek would
 * turn into an episode change. The "centre" layout has no episode
 * buttons at any size; its cluster's flanks are ten-second skips.
 *
 * `cueLift` is how far the subtitles rise while the chrome shows, so
 * the bottom controls never cover them: the bottom chrome's height,
 * less the room Chromium already leaves under the last line.
 */

export type MiniLayout = 'bar' | 'centre';

export const MINI_COMPACT_BELOW = 400;

export interface MiniControlSet {
	compact: boolean;
	/** Whether previous / next have buttons. */
	episodeSteps: 'buttons' | 'hidden';
	/** The centre cluster's flanks: ten-second skips or episode steps. */
	clusterFlanks: 'skip' | 'episode' | null;
	/** The time readout: current and total, or current alone. */
	time: 'both' | 'current';
	mute: boolean;
	/** px the cues rise while the chrome shows. */
	cueLift: number;
}

/** px the cues rise, by layout: [default, compact]. */
const CUE_LIFT = { bar: [56, 36], centre: [32, 24] } as const;

export function miniControlSet(width: number, layout: MiniLayout): MiniControlSet {
	const compact = width < MINI_COMPACT_BELOW;
	return {
		compact,
		episodeSteps: compact || layout === 'centre' ? 'hidden' : 'buttons',
		clusterFlanks: layout === 'centre' ? 'skip' : null,
		time: compact ? 'current' : 'both',
		mute: !compact,
		cueLift: CUE_LIFT[layout][compact ? 1 : 0]
	};
}

/**
 * Whether a cue rises with the chrome. Only cues the track leaves at
 * the bottom do: `line` 'auto', a negative line number (counted from
 * the bottom), or a position at or below the middle. A cue the track
 * places in the upper half — a sign, a second speaker — stays put;
 * lifting it would push it out of the window's top edge.
 */
export function cueLifts(line: number | 'auto', snapToLines: boolean): boolean {
	if (line === 'auto') return true;
	return snapToLines ? line < 0 : line >= 50;
}

/**
 * The subtitle size floor for the mini window: about Chromium's own
 * 5 % of the picture's height, but never under 14 px, where cues stop
 * being readable (Chromium's would be 9 px at 180 tall), and never
 * over 30.
 */
export function cueSizePx(height: number): number {
	return Math.round(Math.max(14, Math.min(30, height * 0.056)));
}
