/**
 * The mini window's keys: the main player's map
 * ($lib/play/keyboard), with F returning playback to the app instead
 * of fullscreen, and Escape closing an open menu and otherwise doing
 * nothing — a stray Escape must not end playback. Nothing here closes
 * the window.
 *
 * `isMiniShortcut` is the main player's key into the mini player,
 * under the same suppression rules as its other letter keys.
 */
import type { PlayerKeyAction, PlayerKeyContext } from '$lib/play/keyboard';

export type MiniKeyAction =
	| Exclude<PlayerKeyAction, { kind: 'fullscreen' }>
	| { kind: 'return' }
	| { kind: 'closeMenu' };

export interface MiniKeyContext extends PlayerKeyContext {
	menuOpen: boolean;
}

export function decideMiniKeyAction(ctx: MiniKeyContext): MiniKeyAction | null {
	void ctx;
	return null;
}

export function isMiniShortcut(ctx: PlayerKeyContext): boolean {
	void ctx;
	return false;
}
