/**
 * What the player frame shows over the picture from a stream attaching
 * to its first frame, and in a stall: the start veil's mode, and the
 * small readings its card uses. The component draws; this decides.
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

/** The veil's mode. An error panel, or a <video> fullscreen on its own,
 *  leaves nothing for it to draw on. A resume hold hides the picture
 *  whatever the controls, so it is always the veil's. A switch is the
 *  page overlay's while windowed; in fullscreen that overlay is out of
 *  the top layer, and the veil shows the next episode. A start or a
 *  stall is the veil's with the custom controls; Chromium's own show
 *  their own. */
export function veilMode(input: VeilInput): VeilMode {
	if (input.playerError || input.videoFullscreen) return 'off';
	if (input.resumeHolding) return 'start';
	if (input.switchBusy && input.fullscreen) return 'start';
	if (!input.customControls) return 'off';
	if (input.phase === 'starting') return 'start';
	if (input.phase === 'stalled') return 'stall';
	return 'off';
}

/** A playback position as the player's time readout reads it: m:ss
 *  under an hour, h:mm:ss from there. */
export function clock(seconds: number): string {
	const s = Number.isFinite(seconds) ? Math.max(0, Math.floor(seconds)) : 0;
	const pad = (n: number) => String(n).padStart(2, '0');
	const h = Math.floor(s / 3600);
	const m = Math.floor((s % 3600) / 60);
	return h > 0 ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

/** The title card's numeral: a whole episode under ten pads to two
 *  digits ("03"); anything else, a special such as 7.5 included, reads
 *  as it is. */
export function episodeNumeral(episode: number): string {
	return Number.isInteger(episode) && episode >= 0 && episode < 10
		? `0${episode}`
		: String(episode);
}
