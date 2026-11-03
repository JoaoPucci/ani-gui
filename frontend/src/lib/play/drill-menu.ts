/**
 * The play page's drill-down menus — the ⋯ menu below the player and
 * the in-player menu — as a pure state machine: whether the menu is
 * open, which list (if any) shows in place of its contents, the width
 * it keeps while the list shows, and where focus goes after each step.
 *
 * The page is the glue: it measures the width, waits for the DOM, and
 * applies the focus target this answers with.
 */

import { menuArrow } from './menu-arrow';

export interface DrillMenu<L extends string> {
	readonly open: boolean;
	/** The list showing in place of the menu's contents; null for the
	 *  menu's main list. Never set while the menu is closed. */
	readonly list: L | null;
	/** The width the menu keeps while a list shows — the main list's,
	 *  so a menu anchored at one edge does not jump as its contents
	 *  swap; null when it sizes itself. */
	readonly width: number | null;
}

/** Where focus goes once the menu has redrawn. */
export type DrillFocus<L extends string> =
	/** The first enabled item: the main list's, or a list's back row. */
	| { readonly kind: 'first' }
	/** The row that opens `list`, on the main list. */
	| { readonly kind: 'opener'; readonly list: L }
	/** The menu's own button. */
	| { readonly kind: 'anchor' }
	/** The first enabled item, or the menu's button when none is. */
	| { readonly kind: 'first-or-anchor' };

export interface DrillStep<L extends string> {
	readonly menu: DrillMenu<L>;
	/** Null where focus stays where it is. */
	readonly focus: DrillFocus<L> | null;
}

export type DrillEvent<L extends string> =
	/** The menu's button: open onto the main list, or close. */
	| { readonly type: 'toggle' }
	/** Close, focus left alone: a click outside, or an item that acts. */
	| { readonly type: 'close' }
	/** Close, focus back on the menu's button: a pick, or Escape. */
	| { readonly type: 'dismiss' }
	/** Open `list` from its row, when it has something to choose. */
	| {
			readonly type: 'openList';
			readonly list: L;
			readonly choosable: boolean;
			readonly width: number | null;
	  }
	/** Back from the list to the main list. */
	| { readonly type: 'back' }
	/** Back from a list, or dismiss from the main list. */
	| { readonly type: 'escape' }
	/** The lists' models changed: a list showing that has nothing left
	 *  to choose closes. */
	| { readonly type: 'models'; readonly choosable: (list: L) => boolean };

/** A closed menu. */
export function closedDrill<L extends string>(): DrillMenu<L> {
	return { open: false, list: null, width: null };
}

const mainList = <L extends string>(open: boolean): DrillMenu<L> => ({
	open,
	list: null,
	width: null
});

/** The menu closed: itself when it already is, so a step that changes
 *  nothing hands back the same object. */
const closedFrom = <L extends string>(menu: DrillMenu<L>): DrillMenu<L> =>
	!menu.open && menu.list === null && menu.width === null ? menu : mainList(false);

/** The menu after `event`, and where focus goes. A step that changes
 *  nothing returns the menu it was given, so a caller comparing by
 *  identity sees no change. */
export function drillStep<L extends string>(
	menu: DrillMenu<L>,
	event: DrillEvent<L>
): DrillStep<L> {
	switch (event.type) {
		case 'toggle':
			return menu.open
				? { menu: mainList(false), focus: null }
				: { menu: mainList(true), focus: { kind: 'first' } };
		case 'close':
			return { menu: closedFrom(menu), focus: null };
		case 'dismiss':
			return { menu: closedFrom(menu), focus: { kind: 'anchor' } };
		case 'openList':
			if (!menu.open || menu.list !== null || !event.choosable) return { menu, focus: null };
			return {
				menu: { open: true, list: event.list, width: event.width },
				focus: { kind: 'first' }
			};
		case 'back':
			if (menu.list === null) return { menu, focus: null };
			return { menu: mainList(true), focus: { kind: 'opener', list: menu.list } };
		case 'escape':
			return menu.list === null
				? drillStep(menu, { type: 'dismiss' })
				: drillStep(menu, { type: 'back' });
		case 'models':
			if (menu.list === null || event.choosable(menu.list)) return { menu, focus: null };
			return { menu: mainList(true), focus: { kind: 'first-or-anchor' } };
	}
}

/** A key pressed in the menu, as far as the menu's state goes: Left and
 *  Right open the list from its row and go back from it, mirrored where
 *  the page reads right to left, and otherwise stay in the menu (an
 *  unchanged step, so the key does not reach the player); null for any
 *  other key, which the menu does not take. */
export function drillArrow<L extends string>(
	menu: DrillMenu<L>,
	key: string,
	at: {
		readonly rtl: boolean;
		/** The list the focused row opens; null when it opens none. */
		readonly opener: L | null;
		/** Whether that list has something to choose. */
		readonly choosable: boolean;
		readonly width: number | null;
	}
): DrillStep<L> | null {
	const arrow = menuArrow(key, {
		rtl: at.rtl,
		listOpen: menu.list !== null,
		onOpener: at.opener !== null
	});
	if (arrow === null) return null;
	if (arrow === 'open' && at.opener !== null) {
		return drillStep(menu, {
			type: 'openList',
			list: at.opener,
			choosable: at.choosable,
			width: at.width
		});
	}
	if (arrow === 'back') return drillStep(menu, { type: 'back' });
	return { menu, focus: null };
}
