/**
 * The DOM side of the app's popup menus: their items, and the steps
 * `$lib/menu/menu-keys` answers, applied to a menu element. A
 * component wires these to its menu and supplies its button and how
 * it closes.
 */

import { menuKeyStep } from '../play/caption-menu-fit';
import { menuKey, refocusAfterChange, typed, type MenuKeyStep } from './menu-keys';

/** A menu's items that take focus: every menuitem role, enabled. */
const ITEMS = '[role^="menuitem"]:not(:disabled)';

/** The menu's enabled items, in order; none for a menu not there. */
export function menuItems(menu: ParentNode | null): HTMLElement[] {
	return menu ? Array.from(menu.querySelectorAll<HTMLElement>(ITEMS)) : [];
}

/** The place of the focused item among the menu's enabled items; null
 *  while focus is on none of them. */
export function focusedIndex(menu: ParentNode | null): number | null {
	const at = menuItems(menu).indexOf(document.activeElement as HTMLElement);
	return at === -1 ? null : at;
}

/** Focuses the menu's first enabled item, if it has one. */
export function focusFirstItem(menu: ParentNode | null): void {
	menuItems(menu)[0]?.focus();
}

/** ArrowDown, ArrowUp, Home and End move focus between the menu's
 *  enabled items, wrapping at the ends, and go no further; any other
 *  key passes. The menu is the event's current target. */
export function stepMenuFocus(e: KeyboardEvent): void {
	const items = menuItems(e.currentTarget as HTMLElement);
	const next = menuKeyStep(
		e.key,
		items.indexOf(document.activeElement as HTMLElement),
		items.length
	);
	if (next === null) return;
	e.preventDefault();
	e.stopPropagation();
	items[next].focus();
}

export interface MenuKeyTarget {
	/** The menu's button, which focus returns to. */
	readonly anchor: HTMLElement | null;
	/** Closes the menu. The components take the menu out of the page
	 *  before returning (Svelte's `flushSync`), so that for Tab nothing
	 *  of it is left for the browser to move into from the button. */
	close(): void;
}

/** Applies `step` to the menu's `items`. */
function apply(e: KeyboardEvent, step: MenuKeyStep, items: HTMLElement[], target: MenuKeyTarget) {
	if (step.kind === 'leave') {
		target.anchor?.focus();
		target.close();
		return;
	}
	e.preventDefault();
	e.stopPropagation();
	if (step.kind === 'focus') items[step.index].focus();
	else if (step.kind === 'dismiss') {
		target.close();
		target.anchor?.focus();
	}
}

/** A key pressed in a popup menu — the event's current target — as
 *  `menuKey` answers it. */
export function onMenuKey(e: KeyboardEvent, target: MenuKeyTarget): void {
	const items = menuItems(e.currentTarget as HTMLElement);
	const step = menuKey(e, items.indexOf(document.activeElement as HTMLElement), items.length);
	if (step !== null) apply(e, step, items, target);
}

/** Tab, Shift+Tab and typed characters in one of the play page's menus,
 *  whose arrows and Escape follow rules of their own: answered as
 *  `onMenuKey` answers them. True when the key was the menu's. */
export function onMenuTabOrType(e: KeyboardEvent, target: MenuKeyTarget): boolean {
	if (e.key !== 'Tab' && !typed(e.key)) return false;
	const step = menuKey(e, -1, 0);
	if (step === null) return false;
	apply(e, step, [], target);
	return true;
}

/** What takes focus from the keyboard. */
const TABBABLE =
	'a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

/** Focuses the first control after `el` in the page's order, outside
 *  it: where Tab would go from `el`'s end. Nothing when none follows. */
export function focusAfter(el: Element): void {
	const next = Array.from(document.querySelectorAll<HTMLElement>(TABBABLE)).find(
		(c) => !el.contains(c) && !!(el.compareDocumentPosition(c) & Node.DOCUMENT_POSITION_FOLLOWING)
	);
	next?.focus();
}

/** After the menu's items changed while it was open: when the item
 *  focus was on (`was`, from `focusedIndex` before the change) went
 *  with the change, focus moves to the item now at its place, the last
 *  one, or the menu's button when none is left. */
export function refocusMenu(
	menu: HTMLElement | null,
	was: number | null,
	anchor: HTMLElement | null
): void {
	const items = menuItems(menu);
	const stillIn = !!menu && menu.contains(document.activeElement);
	const to = refocusAfterChange(was, stillIn, items.length);
	if (to === 'anchor') anchor?.focus();
	else if (to !== null) items[to].focus();
}
