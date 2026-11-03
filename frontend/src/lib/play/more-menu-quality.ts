/**
 * The quality entry of the play page's ⋯ menu, which carries it while
 * the app's own player controls are off: Chromium's native controls
 * have no quality menu, and the in-player one goes with the custom
 * controls. With those on, the in-player menu is the one picker, and
 * the ⋯ menu offers none.
 *
 * The entry is the in-player menu's model (`./quality-menu`), not a
 * second one: the same rows, the same choice in force, and a pick
 * applied through the same quality control.
 */

import type { QualityMenu } from './quality-menu';

/** The ⋯ menu's Quality row. */
export interface MoreQuality {
	readonly menu: QualityMenu;
	/** Whether the row opens the list of levels: there is more than one
	 *  entry to choose between. */
	readonly choosable: boolean;
}

/** The row, or null when the ⋯ menu offers none: the custom controls
 *  carry the picker, or the quality menu has nothing to show. */
export function moreMenuQuality(
	menu: QualityMenu | null,
	customControls: boolean
): MoreQuality | null {
	if (customControls || !menu) return null;
	return { menu, choosable: menu.items.length > 1 };
}

/** What Escape does in the ⋯ menu: leave the list of levels for the
 *  menu's main list, or close the menu. */
export function moreMenuEscape(qualityOpen: boolean): 'back' | 'close' {
	return qualityOpen ? 'back' : 'close';
}

/** Where the ⋯ menu's focus is, for a horizontal arrow key. */
export interface MoreMenuArrowAt {
	/** The page reads right to left. */
	readonly rtl: boolean;
	/** The list of levels is open. */
	readonly qualityOpen: boolean;
	/** Focus is on the Quality row that opens it. */
	readonly onQualityRow: boolean;
}

/** What a horizontal arrow key does in the ⋯ menu: open the levels
 *  from the Quality row, go back from them, or nothing — kept in the
 *  menu, so it does not seek; null for any other key. */
export function moreMenuArrow(key: string, at: MoreMenuArrowAt): 'open' | 'back' | 'stay' | null {
	if (key !== 'ArrowLeft' && key !== 'ArrowRight') return null;
	// Into the line is Right, or Left where the page reads right to left.
	const inward = (key === 'ArrowRight') !== at.rtl;
	if (inward && !at.qualityOpen && at.onQualityRow) return 'open';
	if (!inward && at.qualityOpen) return 'back';
	return 'stay';
}
