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
import { decidePlayerKeyAction } from '$lib/play/keyboard';
import type { PlayerKeyAction, PlayerKeyContext } from '$lib/play/keyboard';

export type MiniKeyAction =
	| Exclude<PlayerKeyAction, { kind: 'fullscreen' }>
	| { kind: 'return' }
	| { kind: 'closeMenu' };

export interface MiniKeyContext extends PlayerKeyContext {
	menuOpen: boolean;
}

export function decideMiniKeyAction(ctx: MiniKeyContext): MiniKeyAction | null {
	if (ctx.key === 'Escape') return ctx.menuOpen ? { kind: 'closeMenu' } : null;
	const action = decidePlayerKeyAction(ctx);
	if (action?.kind === 'fullscreen') return { kind: 'return' };
	return action;
}

export function isMiniShortcut(ctx: PlayerKeyContext): boolean {
	if (ctx.inField || ctx.modifier || ctx.repeat) return false;
	return ctx.key === 'i' || ctx.key === 'I';
}
