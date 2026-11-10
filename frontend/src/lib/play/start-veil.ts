/**
 * What the player frame shows over the picture from a stream attaching
 * to its first frame, and in a stall: the start veil's mode, and the
 * small readings its card uses. The component draws; this decides.
 *
 * The signatures only, so far: the behaviour comes with its tests.
 */

import type { StartPhase } from './start-state';

export type VeilMode = 'off' | 'start' | 'stall';

export interface VeilInput {
	phase: StartPhase;
	resumeHolding: boolean;
	switchBusy: boolean;
	/** The player frame is fullscreen. */
	fullscreen: boolean;
	/** The <video> itself is fullscreen: nothing the frame draws shows. */
	videoFullscreen: boolean;
	playerError: boolean;
	customControls: boolean;
}

export function veilMode(input: VeilInput): VeilMode {
	void input;
	return 'off';
}

export function clock(seconds: number): string {
	void seconds;
	return '';
}

export function episodeNumeral(episode: number): string {
	return String(episode);
}
