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
