/**
 * How the mini window speaks about staying on top.
 *
 * On Windows and X11 the app asks for "always on top" and gets it; a
 * pin in the strip, where shown, toggles it. On native Wayland no app
 * can ask (no protocol exists; Electron documents setAlwaysOnTop as
 * unsupported there), so the window says the desktop decides and how
 * to pin it from the window menu: once, as a card at first open,
 * and/or behind the pin, which then explains instead of toggling.
 */

export type MiniPlatform = 'wayland' | 'windows' | 'x11';
export type OnTopChoice = 'card' | 'pin' | 'both';

export interface OnTopView {
	/** The one-time card shows now. */
	card: boolean;
	/** The strip's pin: a real toggle, an explainer, or none. */
	pin: 'toggle' | 'explain' | null;
}

export function onTopView(
	platform: MiniPlatform,
	choice: OnTopChoice,
	hintSeen: boolean
): OnTopView {
	const wayland = platform === 'wayland';
	const withPin = choice !== 'card';
	return {
		card: wayland && choice !== 'pin' && !hintSeen,
		pin: withPin ? (wayland ? 'explain' : 'toggle') : null
	};
}
