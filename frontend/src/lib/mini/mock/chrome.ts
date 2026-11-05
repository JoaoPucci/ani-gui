/**
 * When the mini window's chrome shows. At rest the window is the
 * picture and its subtitles; pointer movement or keyboard focus brings
 * the chrome, which leaves after MINI_IDLE_HIDE_MS without either.
 * The main player's fullscreen rule ($lib/play/fullscreen-idle), with
 * two more reasons to stay: a menu open, and a card on the picture
 * (the on-top hint, an error, the end of the series) that has buttons.
 */

export const MINI_IDLE_HIDE_MS = 2500;
/** The chrome's fade, in and out; none under reduced motion. */
export const MINI_FADE_MS = 140;

export interface MiniChromeState {
	/** The pointer moved over the window within the idle time. */
	pointerActive: boolean;
	focusWithin: boolean;
	paused: boolean;
	timelineHover: boolean;
	menuOpen: boolean;
	/** A card with its own buttons is on the picture. */
	cardShown: boolean;
}

export function miniChromeShown(state: MiniChromeState): boolean {
	return (
		state.pointerActive ||
		state.focusWithin ||
		state.paused ||
		state.timelineHover ||
		state.menuOpen ||
		state.cardShown
	);
}

export function miniFadeMs(reduced: boolean): number {
	return reduced ? 0 : MINI_FADE_MS;
}

/**
 * The strip, alone, stays up through a stall: it carries the
 * "Reconnecting" status where the title was, and the way back or out.
 * The rest of the chrome follows the usual rule.
 */
export function miniStripShown(chromeShown: boolean, stalled: boolean): boolean {
	return chromeShown || stalled;
}

/**
 * The corner resize grip, drawn only where the platform gives a
 * frameless window no resize edges of its own: Wayland, before
 * Electron 41. There it shows with the chrome, or always.
 */
export function miniGripShown(input: {
	platform: 'wayland' | 'windows' | 'x11';
	grip: 'chrome' | 'always';
	chromeShown: boolean;
}): boolean {
	if (input.platform !== 'wayland') return false;
	return input.grip === 'always' || input.chromeShown;
}
