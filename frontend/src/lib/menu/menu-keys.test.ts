import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { menuKey, refocusAfterChange } from './menu-keys';

const chord = (
	key: string,
	mods: Partial<Record<'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey', boolean>> = {}
) => ({ key, altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, ...mods });

const VERTICAL = ['ArrowDown', 'ArrowUp', 'Home', 'End'] as const;

/** A key that types one character, Space aside. */
const typed = (k: string) => [...k].length === 1 && k !== ' ';

describe('menuKey', () => {
	it('moves down and up through the items, wrapping at the ends', () => {
		expect(menuKey(chord('ArrowDown'), 0, 3)).toEqual({ kind: 'focus', index: 1 });
		expect(menuKey(chord('ArrowDown'), 2, 3)).toEqual({ kind: 'focus', index: 0 });
		expect(menuKey(chord('ArrowUp'), 0, 3)).toEqual({ kind: 'focus', index: 2 });
		expect(menuKey(chord('ArrowUp'), 2, 3)).toEqual({ kind: 'focus', index: 1 });
	});

	it('goes to the first item with Home and the last with End', () => {
		expect(menuKey(chord('Home'), 2, 3)).toEqual({ kind: 'focus', index: 0 });
		expect(menuKey(chord('End'), 0, 3)).toEqual({ kind: 'focus', index: 2 });
	});

	it('enters the items from outside them: Down at the first, Up at the last', () => {
		expect(menuKey(chord('ArrowDown'), -1, 3)).toEqual({ kind: 'focus', index: 0 });
		expect(menuKey(chord('ArrowUp'), -1, 3)).toEqual({ kind: 'focus', index: 2 });
	});

	it('keeps a vertical key in a menu with no items, so it does not scroll the page or reach the player', () => {
		for (const key of VERTICAL) expect(menuKey(chord(key), -1, 0)).toEqual({ kind: 'hold' });
	});

	it('keeps the horizontal arrows in the menu: nothing to open, and the player would seek', () => {
		expect(menuKey(chord('ArrowLeft'), 1, 3)).toEqual({ kind: 'hold' });
		expect(menuKey(chord('ArrowRight'), 1, 3)).toEqual({ kind: 'hold' });
	});

	it('dismisses with Escape, and with Shift+Tab, which leaves toward the menu’s button', () => {
		expect(menuKey(chord('Escape'), 0, 3)).toEqual({ kind: 'dismiss' });
		expect(menuKey(chord('Tab', { shiftKey: true }), 0, 3)).toEqual({ kind: 'dismiss' });
	});

	it('leaves with Tab, focus moving on past the menu’s button', () => {
		expect(menuKey(chord('Tab'), 0, 3)).toEqual({ kind: 'leave' });
	});

	it('reads Shift+arrows as the arrows, as the player does', () => {
		expect(menuKey(chord('ArrowDown', { shiftKey: true }), 0, 3)).toEqual({
			kind: 'focus',
			index: 1
		});
		expect(menuKey(chord('ArrowLeft', { shiftKey: true }), 0, 3)).toEqual({ kind: 'hold' });
	});

	it('leaves any key with Alt, Ctrl or Meta held to the browser', () => {
		fc.assert(
			fc.property(
				fc.constantFrom(...VERTICAL, 'ArrowLeft', 'ArrowRight', 'Escape', 'Tab', 'Enter', ' ', 'a'),
				fc.constantFrom('altKey', 'ctrlKey', 'metaKey'),
				fc.boolean(),
				fc.integer({ min: -1, max: 9 }),
				fc.integer({ min: 0, max: 10 }),
				(key, held, shiftKey, at, count) => {
					expect(menuKey(chord(key, { [held]: true, shiftKey }), at, count)).toBeNull();
				}
			)
		);
	});

	it('leaves every key it does not name to the browser: Enter and Space press the item', () => {
		const named = new Set<string>([...VERTICAL, 'ArrowLeft', 'ArrowRight', 'Escape', 'Tab']);
		fc.assert(
			fc.property(
				fc.string().filter((k) => !named.has(k) && !typed(k)),
				fc.integer({ min: -1, max: 9 }),
				fc.integer({ min: 0, max: 10 }),
				(key, at, count) => {
					expect(menuKey(chord(key), at, count)).toBeNull();
				}
			)
		);
		expect(menuKey(chord('Enter'), 0, 3)).toBeNull();
		expect(menuKey(chord(' '), 0, 3)).toBeNull();
	});

	it('keeps a typed character in the menu: on the play page n, f and m are player keys', () => {
		for (const key of ['n', 'f', 'm', 'M', '1', 'é', '😀']) {
			expect(menuKey(chord(key), 0, 3)).toEqual({ kind: 'hold' });
		}
		fc.assert(
			fc.property(
				fc.string({ unit: 'grapheme', minLength: 1, maxLength: 1 }).filter((k) => typed(k)),
				fc.boolean(),
				(key, shiftKey) => {
					expect(menuKey(chord(key, { shiftKey }), 0, 3)).toEqual({ kind: 'hold' });
				}
			)
		);
	});

	it('always lands a vertical key on an item that exists', () => {
		fc.assert(
			fc.property(
				fc.constantFrom(...VERTICAL),
				fc.integer({ min: 1, max: 50 }),
				fc.nat(),
				(key, count, seed) => {
					const at = (seed % (count + 1)) - 1;
					const step = menuKey(chord(key), at, count);
					expect(step?.kind).toBe('focus');
					const index = (step as { index: number }).index;
					expect(index).toBeGreaterThanOrEqual(0);
					expect(index).toBeLessThan(count);
				}
			)
		);
	});

	it('visits every item once in a round of Down presses, and Up retraces it', () => {
		fc.assert(
			fc.property(fc.integer({ min: 1, max: 30 }), (count) => {
				const seen: number[] = [];
				let at = -1;
				for (let i = 0; i < count; i++) {
					at = (menuKey(chord('ArrowDown'), at, count) as { index: number }).index;
					seen.push(at);
				}
				expect(seen).toEqual(Array.from({ length: count }, (_, i) => i));
				for (let i = count - 1; i > 0; i--) {
					at = (menuKey(chord('ArrowUp'), at, count) as { index: number }).index;
					expect(at).toBe(i - 1);
				}
			})
		);
	});
});

describe('refocusAfterChange', () => {
	it('leaves focus alone when it was not on an item before the change', () => {
		expect(refocusAfterChange(null, false, 3)).toBeNull();
		expect(refocusAfterChange(null, false, 0)).toBeNull();
	});

	it('leaves focus alone when it is still in the menu', () => {
		expect(refocusAfterChange(1, true, 3)).toBeNull();
	});

	it('puts focus back at the place it was when its item went', () => {
		expect(refocusAfterChange(1, false, 3)).toBe(1);
	});

	it('takes the last item when the one that went was at the end', () => {
		expect(refocusAfterChange(4, false, 2)).toBe(1);
	});

	it('hands focus to the menu’s button when no item is left', () => {
		expect(refocusAfterChange(0, false, 0)).toBe('anchor');
	});

	it('answers an item that exists, or the button when none does', () => {
		fc.assert(
			fc.property(fc.nat(40), fc.nat(40), (was, count) => {
				const to = refocusAfterChange(was, false, count);
				if (count === 0) expect(to).toBe('anchor');
				else {
					expect(typeof to).toBe('number');
					expect(to as number).toBeLessThan(count);
					expect(to as number).toBeLessThanOrEqual(was);
				}
			})
		);
	});
});
