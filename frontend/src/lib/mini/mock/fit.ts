/**
 * Which controls the mini window draws at a width. At the default
 * 480 px and up every control has a place; below MINI_COMPACT_BELOW
 * (the 320 px minimum among them) the bottom row keeps play, the
 * timeline and captions, and previous / next move: into the
 * timeline's hover in the "bar" layout, out of the centre cluster's
 * flanks in the "centre" layout, where the ten-second skips stay.
 */

export type MiniLayout = 'bar' | 'centre';

export const MINI_COMPACT_BELOW = 400;

export interface MiniControlSet {
	compact: boolean;
	/** Where previous / next sit: their own buttons, or the timeline's hover. */
	episodeSteps: 'buttons' | 'timeline';
	/** The centre cluster's flanks: ten-second skips or episode steps. */
	clusterFlanks: 'skip' | 'episode' | null;
	/** The time readout: current and total, or current alone. */
	time: 'both' | 'current';
	mute: boolean;
}

export function miniControlSet(width: number, layout: MiniLayout): MiniControlSet {
	const compact = width < MINI_COMPACT_BELOW;
	return {
		compact,
		episodeSteps: compact ? 'timeline' : 'buttons',
		clusterFlanks: layout === 'centre' ? 'skip' : null,
		time: compact ? 'current' : 'both',
		mute: !compact
	};
}
