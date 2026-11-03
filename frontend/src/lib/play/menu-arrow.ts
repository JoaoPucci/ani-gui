/**
 * What a horizontal arrow key does in one of the play page's menus
 * that open a list in place of their contents — the ⋯ menu's Quality
 * row, the in-player menu's Speed and Quality rows: open the list from
 * the row that opens it, go back from the list, or nothing, kept in
 * the menu so that it does not reach the player's seek keys.
 */

/** Where the menu's focus is, for a horizontal arrow key. */
export interface MenuArrowAt {
	/** The page reads right to left. */
	readonly rtl: boolean;
	/** A list the menu opened is showing in place of its contents. */
	readonly listOpen: boolean;
	/** Focus is on a row that opens a list. */
	readonly onOpener: boolean;
}

/** `open` from a row that opens a list, `back` from the list, `stay`
 *  for any other horizontal arrow; null for any other key. */
export function menuArrow(key: string, at: MenuArrowAt): 'open' | 'back' | 'stay' | null {
	if (key !== 'ArrowLeft' && key !== 'ArrowRight') return null;
	// Into the line is Right, or Left where the page reads right to left.
	const inward = (key === 'ArrowRight') !== at.rtl;
	if (inward && !at.listOpen && at.onOpener) return 'open';
	if (!inward && at.listOpen) return 'back';
	return 'stay';
}

/** A key event's modifiers and key, as the menus read them. */
export interface KeyChord {
	readonly key: string;
	readonly altKey: boolean;
	readonly ctrlKey: boolean;
	readonly metaKey: boolean;
	readonly shiftKey: boolean;
}

/** The key a menu acts on: the event's key unless Alt, Ctrl or Meta is
 *  held, null then — such a key (Alt+ArrowLeft for history back, and
 *  the like) is not the menu's, and passes through untouched. */
export function plainKey(chord: KeyChord): string | null {
	// Shift is read as the key itself, as the player's own keys read it:
	// passed on, Shift+arrows would reach the player's volume and seek.
	const held = chord.altKey || chord.ctrlKey || chord.metaKey;
	return held ? null : chord.key;
}
