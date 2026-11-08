import { describe, expect, it } from 'vitest';
import {
	guessBinds,
	guessedId,
	guessRow,
	openedFromGuess,
	openedFromRow,
	recordableId,
	withGuess,
	withRow
} from './play-origin';

describe('a play session opened from a guessed Continue match', () => {
	it('is told by the guess flag in its URL', () => {
		expect(openedFromGuess(new URLSearchParams('session=s&episode=3&guess=1'))).toBe(true);
		expect(openedFromGuess(new URLSearchParams('session=s&episode=3'))).toBe(false);
		expect(openedFromGuess(new URLSearchParams('guess=0'))).toBe(false);
	});

	it('records no Kitsu id, and any other session records its own', () => {
		expect(recordableId('42', true)).toBeUndefined();
		expect(recordableId('42', false)).toBe('42');
		expect(recordableId('', false)).toBeUndefined();
	});

	it("records its play once the play landed on the card's own listing", () => {
		// The backend says so when the play names the card's row
		// (`guess_row`) and lands on that row's listing.
		expect(guessBinds(true, { own_listing: true })).toBe(true);
		expect(guessBinds(true, { own_listing: false })).toBe(false);
		expect(guessBinds(true, {})).toBe(false);
		// A session no guess opened records its play as it always did.
		expect(guessBinds(false, {})).toBe(true);
	});

	it("names the card's row on its plays, and no other session does", () => {
		expect(guessRow(true, 'hianime:show-1')).toBe('hianime:show-1');
		expect(guessRow(false, 'hianime:show-1')).toBeUndefined();
		expect(guessRow(true, null)).toBeUndefined();
	});

	it('names its guess apart from the page on its plays, and no other session does', () => {
		// The backend records it on the resolve's row only when the play
		// lands on the card's own listing; it is never the play's page.
		expect(guessedId(true, '42')).toBe('42');
		expect(guessedId(false, '42')).toBeUndefined();
		expect(guessedId(true, '')).toBeUndefined();
	});

	it('carries the flag on every URL it builds for itself', () => {
		const q = withGuess('?session=s&episode=3', true);
		expect(new URLSearchParams(q.slice(1)).get('guess')).toBe('1');
		expect(new URLSearchParams(q.slice(1)).get('session')).toBe('s');
		expect(withGuess('?session=s&episode=3', false)).toBe('?session=s&episode=3');
	});
});

describe('a play session opened from a Continue card', () => {
	it('carries the history row on every URL it builds for itself', () => {
		const q = withRow('?session=s&episode=3&guess=1', 'hianime:show-1');
		const params = new URLSearchParams(q.slice(1));
		expect(openedFromRow(params)).toBe('hianime:show-1');
		expect(params.get('guess')).toBe('1');
		expect(withRow('?', 'r')).toBe('?row=r');
	});

	it('names no row when a detail page opened it', () => {
		expect(openedFromRow(new URLSearchParams('session=s&episode=3'))).toBeNull();
		expect(openedFromRow(new URLSearchParams('row='))).toBeNull();
		expect(withRow('?session=s', null)).toBe('?session=s');
	});
});
