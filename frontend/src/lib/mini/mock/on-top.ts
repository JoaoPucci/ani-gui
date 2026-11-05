/**
 * How the mini window speaks about staying on top.
 *
 * On Windows and X11 the app asks for "always on top" and gets it; a
 * pin in the strip, where shown, toggles it. On native Wayland no app
 * can ask (no protocol exists; Electron documents setAlwaysOnTop as
 * unsupported there), so the window says the desktop decides and how
 * to pin it from the window menu: once by itself at the first open
 * (as a card over the picture, or as the pin's explainer), and/or
 * behind the pin, which then explains instead of toggling.
 */

export type MiniPlatform = 'wayland' | 'windows' | 'x11';
export type OnTopChoice = 'card' | 'pin' | 'both';

export interface OnTopView {
	/** What the first open on Wayland shows by itself, until seen: the
	 *  card over the picture, the pin's explainer, or nothing. */
	firstOpen: 'card' | 'popover' | null;
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
	const first = choice === 'card' ? 'card' : choice === 'both' ? 'popover' : null;
	return {
		firstOpen: wayland && !hintSeen ? first : null,
		pin: withPin ? (wayland ? 'explain' : 'toggle') : null
	};
}

/** Which explainer the Wayland pin shows. */
export type OnTopCopy = 'gnome' | 'kde' | 'other';

/**
 * The explainer for a desktop, from XDG_CURRENT_DESKTOP (a
 * colon-separated list, e.g. "ubuntu:GNOME"). GNOME's names the
 * window menu's "Always on Top" only once someone has confirmed it
 * works on the app's frameless window (step 4 of the mini-player
 * proposal's appendix, PR #345); until then GNOME gets the general
 * copy. KDE gets its window menu's "Keep Above Others".
 */
export function onTopCopy(desktop: string | null, gnomeMenuConfirmed: boolean): OnTopCopy {
	const names = (desktop ?? '').toLowerCase().split(':');
	if (names.includes('gnome') && gnomeMenuConfirmed) return 'gnome';
	if (names.includes('kde')) return 'kde';
	return 'other';
}
