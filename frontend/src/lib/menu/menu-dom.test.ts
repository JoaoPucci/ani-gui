// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';
import {
	focusAfter,
	focusFirstItem,
	focusedIndex,
	menuItems,
	onMenuKey,
	onMenuTabOrType,
	refocusMenu,
	stepMenuFocus
} from './menu-dom';

let anchor: HTMLButtonElement;
let menu: HTMLDivElement;
let after: HTMLButtonElement;

/** A menu button, its menu of `labels` (a label ending in `!` is a
 *  disabled item), and a button after them in the tab order. */
function build(labels: string[]) {
	anchor = document.createElement('button');
	anchor.textContent = 'open';
	menu = document.createElement('div');
	menu.setAttribute('role', 'menu');
	for (const label of labels) {
		const item = document.createElement('button');
		item.setAttribute('role', label.startsWith('radio') ? 'menuitemradio' : 'menuitem');
		item.textContent = label;
		item.disabled = label.endsWith('!');
		menu.appendChild(item);
	}
	// Not an item: text a menu carries beside its items.
	const note = document.createElement('p');
	note.textContent = 'note';
	menu.prepend(note);
	after = document.createElement('button');
	document.body.append(anchor, menu, after);
}

const item = (label: string) =>
	Array.from(menu.querySelectorAll<HTMLElement>('[role^="menuitem"]')).find(
		(el) => el.textContent === label
	)!;

/** Presses `key` on the focused element, the menu listening as a
 *  component wires it; answers the event, to read what it was let do. */
function press(
	key: string,
	handler: (e: KeyboardEvent) => void,
	mods: Partial<Record<'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey', boolean>> = {}
): KeyboardEvent {
	const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...mods });
	const listener = (ev: Event) => handler(ev as KeyboardEvent);
	menu.addEventListener('keydown', listener);
	(document.activeElement ?? document.body).dispatchEvent(e);
	menu.removeEventListener('keydown', listener);
	return e;
}

beforeEach(() => {
	document.body.innerHTML = '';
});

afterEach(() => {
	document.body.innerHTML = '';
});

describe('menuItems', () => {
	it('lists the enabled items of every menuitem role, in order, and nothing else', () => {
		build(['a', 'b!', 'radio c']);
		expect(menuItems(menu).map((el) => el.textContent)).toEqual(['a', 'radio c']);
	});

	it('lists nothing for a menu that is not there', () => {
		expect(menuItems(null)).toEqual([]);
	});
});

describe('focusedIndex', () => {
	it('names the focused item’s place among the enabled items', () => {
		build(['a', 'b!', 'c']);
		item('c').focus();
		expect(focusedIndex(menu)).toBe(1);
	});

	it('is null while focus is on no item of the menu', () => {
		build(['a']);
		after.focus();
		expect(focusedIndex(menu)).toBeNull();
		expect(focusedIndex(null)).toBeNull();
	});
});

describe('focusFirstItem', () => {
	it('focuses the first enabled item', () => {
		build(['a!', 'b', 'c']);
		focusFirstItem(menu);
		expect(document.activeElement).toBe(item('b'));
	});

	it('does nothing for a menu with no enabled item, or none at all', () => {
		build(['a!']);
		anchor.focus();
		focusFirstItem(menu);
		focusFirstItem(null);
		expect(document.activeElement).toBe(anchor);
	});
});

describe('stepMenuFocus', () => {
	it('moves through the enabled items with the arrows, Home and End, wrapping', () => {
		build(['a', 'b!', 'c', 'd']);
		item('a').focus();
		const down = press('ArrowDown', stepMenuFocus);
		expect(document.activeElement).toBe(item('c'));
		expect(down.defaultPrevented).toBe(true);
		press('End', stepMenuFocus);
		expect(document.activeElement).toBe(item('d'));
		press('ArrowDown', stepMenuFocus);
		expect(document.activeElement).toBe(item('a'));
		press('ArrowUp', stepMenuFocus);
		expect(document.activeElement).toBe(item('d'));
		press('Home', stepMenuFocus);
		expect(document.activeElement).toBe(item('a'));
	});

	it('keeps the key from the page’s own listeners once it moved focus', () => {
		build(['a', 'b']);
		item('a').focus();
		const page = vi.fn();
		document.addEventListener('keydown', page);
		press('ArrowDown', stepMenuFocus);
		document.removeEventListener('keydown', page);
		expect(page).not.toHaveBeenCalled();
	});

	it('lets any other key through', () => {
		build(['a', 'b']);
		item('a').focus();
		const e = press('Enter', stepMenuFocus);
		expect(e.defaultPrevented).toBe(false);
		expect(document.activeElement).toBe(item('a'));
	});
});

describe('onMenuKey', () => {
	let close: Mock<() => void>;
	const handler = (e: KeyboardEvent) => onMenuKey(e, { anchor, close });

	beforeEach(() => {
		close = vi.fn<() => void>();
	});

	it('steps through the items, wrapping, as the play page’s menus do', () => {
		build(['a', 'b']);
		item('a').focus();
		press('ArrowDown', handler);
		expect(document.activeElement).toBe(item('b'));
		press('ArrowDown', handler);
		expect(document.activeElement).toBe(item('a'));
		press('End', handler);
		expect(document.activeElement).toBe(item('b'));
		expect(close).not.toHaveBeenCalled();
	});

	it('keeps the horizontal arrows and the vertical keys of an empty menu from the page', () => {
		build(['a']);
		item('a').focus();
		const page = vi.fn();
		document.addEventListener('keydown', page);
		const left = press('ArrowLeft', handler);
		menu.replaceChildren();
		menu.tabIndex = -1;
		menu.focus();
		const down = press('ArrowDown', handler);
		document.removeEventListener('keydown', page);
		expect(page).not.toHaveBeenCalled();
		expect(left.defaultPrevented).toBe(true);
		expect(down.defaultPrevented).toBe(true);
	});

	it('closes on Escape and hands focus back to the menu’s button', () => {
		build(['a', 'b']);
		item('b').focus();
		const page = vi.fn();
		document.addEventListener('keydown', page);
		const e = press('Escape', handler);
		document.removeEventListener('keydown', page);
		expect(close).toHaveBeenCalledOnce();
		expect(document.activeElement).toBe(anchor);
		expect(e.defaultPrevented).toBe(true);
		expect(page).not.toHaveBeenCalled();
	});

	it('closes on Shift+Tab with focus on the menu’s button, which precedes the menu', () => {
		build(['a', 'b']);
		item('a').focus();
		const e = press('Tab', handler, { shiftKey: true });
		expect(close).toHaveBeenCalledOnce();
		expect(document.activeElement).toBe(anchor);
		expect(e.defaultPrevented).toBe(true);
	});

	it('closes on Tab from the menu’s button and lets Tab move on from there', () => {
		build(['a', 'b']);
		item('a').focus();
		let focusAtClose: Element | null = null;
		close.mockImplementation(() => {
			focusAtClose = document.activeElement;
		});
		const e = press('Tab', handler);
		expect(close).toHaveBeenCalledOnce();
		// Focus is on the button before the menu goes, so the browser's
		// Tab moves on from the button rather than from a removed item.
		expect(focusAtClose).toBe(anchor);
		expect(e.defaultPrevented).toBe(false);
	});

	it('leaves modified keys, Enter and Space to the browser', () => {
		build(['a', 'b']);
		item('a').focus();
		for (const [key, mods] of [
			['ArrowDown', { altKey: true }],
			['Escape', { ctrlKey: true }],
			['Tab', { metaKey: true }],
			['Enter', {}],
			[' ', {}]
		] as const) {
			const e = press(key, handler, mods);
			expect(e.defaultPrevented).toBe(false);
		}
		expect(document.activeElement).toBe(item('a'));
		expect(close).not.toHaveBeenCalled();
	});

	it('survives a menu with no button to hand focus back to', () => {
		build(['a']);
		item('a').focus();
		press('Escape', (e) => onMenuKey(e, { anchor: null, close }));
		press('Tab', (e) => onMenuKey(e, { anchor: null, close }));
		expect(close).toHaveBeenCalledTimes(2);
	});
});

describe('refocusMenu', () => {
	it('puts focus on the item now at the place of the one that went', () => {
		build(['a', 'b', 'c']);
		item('b').focus();
		const was = focusedIndex(menu);
		item('b').remove();
		refocusMenu(menu, was, anchor);
		expect(document.activeElement).toBe(item('c'));
	});

	it('takes the last item when the one that went was at the end', () => {
		build(['a', 'b']);
		item('b').focus();
		const was = focusedIndex(menu);
		item('b').remove();
		refocusMenu(menu, was, anchor);
		expect(document.activeElement).toBe(item('a'));
	});

	it('hands focus to the menu’s button when no item is left', () => {
		build(['a']);
		item('a').focus();
		const was = focusedIndex(menu);
		item('a').remove();
		refocusMenu(menu, was, anchor);
		expect(document.activeElement).toBe(anchor);
	});

	it('leaves focus alone while it is still on an item, or was never on one', () => {
		build(['a', 'b']);
		item('b').focus();
		refocusMenu(menu, 1, anchor);
		expect(document.activeElement).toBe(item('b'));
		after.focus();
		refocusMenu(menu, null, anchor);
		expect(document.activeElement).toBe(after);
	});

	it('does nothing without a button when no item is left', () => {
		build(['a']);
		item('a').focus();
		item('a').remove();
		refocusMenu(menu, 0, null);
		expect(document.activeElement).not.toBe(anchor);
	});
});

describe('onMenuTabOrType', () => {
	let close: Mock<() => void>;
	const handler = (e: KeyboardEvent) => {
		taken = onMenuTabOrType(e, { anchor, close });
	};
	let taken = false;

	beforeEach(() => {
		close = vi.fn<() => void>();
		taken = false;
	});

	it('closes on Tab from the menu’s button, Tab moving on from there', () => {
		build(['a', 'b']);
		item('a').focus();
		const e = press('Tab', handler);
		expect(taken).toBe(true);
		expect(close).toHaveBeenCalledOnce();
		expect(document.activeElement).toBe(anchor);
		expect(e.defaultPrevented).toBe(false);
	});

	it('closes on Shift+Tab with focus on the menu’s button', () => {
		build(['a', 'b']);
		item('b').focus();
		const e = press('Tab', handler, { shiftKey: true });
		expect(taken).toBe(true);
		expect(close).toHaveBeenCalledOnce();
		expect(document.activeElement).toBe(anchor);
		expect(e.defaultPrevented).toBe(true);
	});

	it('keeps a typed character from the page', () => {
		build(['a']);
		item('a').focus();
		const page = vi.fn();
		document.addEventListener('keydown', page);
		const e = press('m', handler);
		document.removeEventListener('keydown', page);
		expect(taken).toBe(true);
		expect(e.defaultPrevented).toBe(true);
		expect(page).not.toHaveBeenCalled();
		expect(close).not.toHaveBeenCalled();
	});

	it('leaves the keys the menu steps or closes by its own rule, and modified keys, alone', () => {
		build(['a', 'b']);
		item('a').focus();
		for (const [key, mods] of [
			['ArrowDown', {}],
			['ArrowLeft', {}],
			['Escape', {}],
			['Enter', {}],
			[' ', {}],
			['Tab', { altKey: true }],
			['m', { ctrlKey: true }]
		] as const) {
			const e = press(key, handler, mods);
			expect(taken, key).toBe(false);
			expect(e.defaultPrevented).toBe(false);
		}
		expect(close).not.toHaveBeenCalled();
		expect(document.activeElement).toBe(item('a'));
	});
});

describe('focusAfter', () => {
	it('focuses the first control after the element, outside it', () => {
		build(['a']);
		item('a').focus();
		focusAfter(menu);
		expect(document.activeElement).toBe(after);
	});

	it('skips what cannot take focus from the keyboard', () => {
		build(['a']);
		const disabled = document.createElement('button');
		disabled.disabled = true;
		const unfocusable = document.createElement('span');
		unfocusable.tabIndex = -1;
		after.before(disabled, unfocusable);
		focusAfter(anchor);
		// The menu's own item is after the button, and takes focus.
		expect(document.activeElement).toBe(item('a'));
		focusAfter(menu);
		expect(document.activeElement).toBe(after);
	});

	it('leaves focus alone when nothing follows', () => {
		build(['a']);
		item('a').focus();
		focusAfter(after);
		expect(document.activeElement).toBe(item('a'));
	});
});
