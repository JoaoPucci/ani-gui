import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { dismissKey, focusBack, focusEntered, focusLeft, keyboardClick } from './focus-return';

// Elements as names; the card holds the ones starting with "card:".
const inCard = (el: string) => el.startsWith('card:');
const outside = fc.constantFrom('play', 'seek', 'volume', 'body');
const inside = fc.constantFrom('card:close', 'card:link', 'card:row');
const anyEl = fc.oneof(outside, inside);

describe('focusEntered', () => {
	it('keeps the element outside the card that focus came from', () => {
		expect(focusEntered(null, 'play', inCard)).toBe('play');
		expect(focusEntered('seek', 'play', inCard)).toBe('play');
	});

	it('keeps what it had when focus moves within the card, or comes from nowhere known', () => {
		expect(focusEntered('play', 'card:close', inCard)).toBe('play');
		expect(focusEntered('play', null, inCard)).toBe('play');
		expect(focusEntered(null, null, inCard)).toBeNull();
	});

	it('never keeps an element inside the card', () => {
		fc.assert(
			fc.property(
				fc.option(outside, { nil: null }),
				fc.option(anyEl, { nil: null }),
				(kept, from) => {
					const got = focusEntered(kept, from, inCard);
					expect(got === null || !inCard(got)).toBe(true);
				}
			)
		);
	});
});

describe('focusLeft', () => {
	it('keeps the element while focus moves within the card', () => {
		expect(focusLeft('play', 'card:link', inCard)).toBe('play');
	});

	it('forgets it once focus leaves the card, for an element or for nowhere', () => {
		expect(focusLeft('play', 'seek', inCard)).toBeNull();
		expect(focusLeft('play', null, inCard)).toBeNull();
	});

	it('keeps the element exactly when focus stays in the card', () => {
		fc.assert(
			fc.property(
				fc.option(outside, { nil: null }),
				fc.option(anyEl, { nil: null }),
				(kept, to) => {
					const got = focusLeft(kept, to, inCard);
					expect(got).toBe(to !== null && inCard(to) ? kept : null);
				}
			)
		);
	});
});

describe('focusBack', () => {
	it('gives focus back on a keyboard dismissal made from inside the card', () => {
		expect(focusBack('play', true, true)).toBe('play');
	});

	it('gives it back nowhere on a pointer dismissal, or when focus is not in the card', () => {
		expect(focusBack('play', false, true)).toBeNull();
		expect(focusBack('play', true, false)).toBeNull();
		expect(focusBack('play', false, false)).toBeNull();
		expect(focusBack(null, true, true)).toBeNull();
	});

	it('answers the kept element or nothing', () => {
		fc.assert(
			fc.property(
				fc.option(outside, { nil: null }),
				fc.boolean(),
				fc.boolean(),
				(kept, byKeyboard, focusInCard) => {
					expect(focusBack(kept, byKeyboard, focusInCard)).toBe(
						byKeyboard && focusInCard ? kept : null
					);
				}
			)
		);
	});
});

describe('a card visit', () => {
	it('gives focus back to where a keyboard user came from, after moving within the card', () => {
		let kept: string | null = null;
		kept = focusEntered(kept, 'play', inCard);
		kept = focusLeft(kept, 'card:link', inCard);
		kept = focusEntered(kept, 'card:close', inCard);
		expect(focusBack(kept, keyboardClick(0), true)).toBe('play');
	});

	it('forgets the way back once focus has left, so a later dismissal moves nothing', () => {
		let kept: string | null = null;
		kept = focusEntered(kept, 'play', inCard);
		kept = focusLeft(kept, 'seek', inCard);
		expect(focusBack(kept, true, false)).toBeNull();
	});
});

describe('keyboardClick', () => {
	it('reads a click with no presses as the keyboard, and any press as a pointer', () => {
		expect(keyboardClick(0)).toBe(true);
		expect(keyboardClick(1)).toBe(false);
		expect(keyboardClick(2)).toBe(false);
	});
});

describe('dismissKey', () => {
	it('dismisses on Esc only', () => {
		expect(dismissKey('Escape')).toBe(true);
		expect(dismissKey('Enter')).toBe(false);
		expect(dismissKey(' ')).toBe(false);
		expect(dismissKey('Esc')).toBe(false);
	});
});
