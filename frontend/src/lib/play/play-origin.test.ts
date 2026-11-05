import { describe, expect, it } from 'vitest';
import {
	guessBinds,
	guessedId,
	guessRow,
	landedElsewhere,
	openedFromGuess,
	openedFromRow,
	recordableId,
	sessionShow,
	withGuess,
	withLanding,
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

describe("where a guessed session's play landed", () => {
	it("is carried in the URL only when a guess landed on another listing than the card's", () => {
		expect(withLanding('?session=s', true, { own_listing: false })).toBe('?session=s&landed=other');
		expect(withLanding('?session=s', true, {})).toBe('?session=s&landed=other');
		expect(withLanding('?session=s', true, { own_listing: true })).toBe('?session=s');
		// A session no guess opened lands nowhere in particular.
		expect(withLanding('?session=s', false, { own_listing: false })).toBe('?session=s');
	});

	it('is read back from the URL', () => {
		expect(landedElsewhere(new URLSearchParams('session=s&guess=1&landed=other'))).toBe(true);
		expect(landedElsewhere(new URLSearchParams('session=s&guess=1'))).toBe(false);
	});
});

describe('the show a session plays as', () => {
	const CARD = 'hianime:jojos-bizarre-adventure-2000-5182';

	it('is the page for a session no guess opened', () => {
		expect(sessionShow({ id: '7158', guess: false, row: CARD, linked: null })).toEqual({
			kitsuId: '7158',
			recordAs: '7158',
			mirrors: true
		});
	});

	it('is the guess, named apart, for a guessed session', () => {
		expect(sessionShow({ id: '7158', guess: true, row: CARD, linked: null })).toEqual({
			guessRow: CARD,
			guessId: '7158',
			recordAs: '7158',
			mirrors: true
		});
	});

	it('is the linked show once the card was linked from the session, never the old guess', () => {
		// Later episodes go out naming the link: the backend records it
		// only on the card's own listing, and the trackers hear nothing of
		// a stream that is still the guessed show's.
		expect(sessionShow({ id: '7158', guess: true, row: CARD, linked: '4013' })).toEqual({
			guessRow: CARD,
			guessId: '4013',
			recordAs: '4013',
			mirrors: false
		});
	});
});
