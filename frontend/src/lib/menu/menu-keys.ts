/**
 * What a key does in one of the app's popup menus (`role="menu"`):
 * the account chip's, the download dock's, the Continue row's, and,
 * for the vertical keys, Tab and typed characters, the play page's.
 *
 * Down and Up move through the menu's items and wrap at the ends, Home
 * and End go to the first and last; Left and Right are held in the
 * menu — these menus open nothing sideways, and on the play page the
 * player would take them as seek keys. A typed character is held too,
 * Space aside (it presses the item): n, f and m are the player's. Escape closes the menu with
 * focus back on its button, and so does Shift+Tab, the button being
 * where that key heads; Tab closes it and moves on past the button.
 * A key with Alt, Ctrl or Meta held is the browser's, as it is the
 * player's ($lib/play/menu-arrow's `plainKey`).
 *
 * `$lib/menu/menu-dom` applies the step to a menu element.
 */

import { menuKeyStep } from '../play/caption-menu-fit';
import { plainKey, type KeyChord } from '../play/menu-arrow';

export type MenuKeyStep =
	/** Focus the enabled item at `index`. */
	| { readonly kind: 'focus'; readonly index: number }
	/** Keep the key in the menu, focus where it is. */
	| { readonly kind: 'hold' }
	/** Close, focus back on the menu's button. */
	| { readonly kind: 'dismiss' }
	/** Close, letting Tab move focus on from the menu's button. */
	| { readonly kind: 'leave' };

/** Whether `key` types one character: a key name ('Enter', 'Tab') is
 *  longer. Space is the press of the focused item, and is not one. */
export function typed(key: string): boolean {
	return [...key].length === 1 && key !== ' ';
}

/** The step for `chord` with focus on the enabled item at `at` (-1 for
 *  none) of `count`; null for a key the menu leaves to the browser —
 *  Enter and Space among them, which press the item. */
export function menuKey(chord: KeyChord, at: number, count: number): MenuKeyStep | null {
	const key = plainKey(chord);
	if (key === null) return null;
	if (key === 'Escape') return { kind: 'dismiss' };
	if (key === 'Tab') return chord.shiftKey ? { kind: 'dismiss' } : { kind: 'leave' };
	if (key === 'ArrowLeft' || key === 'ArrowRight' || typed(key)) return { kind: 'hold' };
	const index = menuKeyStep(key, at, count);
	if (index !== null) return { kind: 'focus', index };
	// An empty menu has nowhere to step to, but the vertical keys are
	// still its own: held, so they neither scroll nor reach the player.
	const vertical = menuKeyStep(key, -1, 1) !== null;
	return vertical ? { kind: 'hold' } : null;
}

/** Where focus goes once the menu's items changed while it was open:
 *  `was` is the enabled item focus was on before the change (null when
 *  it was on none), `stillIn` whether focus is still in the menu, and
 *  `count` the enabled items now. The item now at that place, the last
 *  one when the menu got shorter, or the menu's button when no item is
 *  left; null where focus stays where it is. */
export function refocusAfterChange(
	was: number | null,
	stillIn: boolean,
	count: number
): number | 'anchor' | null {
	if (was === null || stillIn) return null;
	return count === 0 ? 'anchor' : Math.min(was, count - 1);
}
