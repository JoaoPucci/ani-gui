// What a play session names as its show.

import fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { sessionShow } from './session-show';

describe('the show a session plays as', () => {
	const CARD = 'hianime:jojos-bizarre-adventure-2000-5182';

	it("is the page for a session no guess opened, naming the card's row it came from", () => {
		// The backend keeps the show the card's line records against the
		// session's plays: a player mounted again after "Keep watching
		// this" sends them as the page's show until it has read the pick.
		expect(sessionShow({ id: '7158', guess: false, row: CARD, linked: null })).toEqual({
			kitsuId: '7158',
			cardRow: CARD,
			recordAs: '7158',
			mirrors: true
		});
		expect(sessionShow({ id: '7158', guess: false, row: null, linked: null })).toEqual({
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

	it('is the show picked again on a session that switched to its first pick', () => {
		// The session plays the show it switched to, from the card's row,
		// and the viewer picked another show for the row from it: the
		// stream is still the first pick's, so the new pick goes out
		// apart, on the card's row, and the trackers hear nothing.
		expect(sessionShow({ id: '4013', guess: false, row: CARD, linked: '9999' })).toEqual({
			guessRow: CARD,
			guessId: '9999',
			recordAs: '9999',
			mirrors: false
		});
	});

	it('names the page as itself whenever nothing was picked apart from it', () => {
		fc.assert(
			fc.property(
				fc.constantFrom('7158', '4013'),
				fc.boolean(),
				fc.option(fc.constant(CARD), { nil: null }),
				fc.option(fc.constantFrom('7158', '4013', '9999'), { nil: null }),
				(id, guess, row, linked) => {
					const as = sessionShow({ id, guess, row, linked });
					// The trackers hear a session only when what plays is what it
					// records.
					expect(as.mirrors).toBe(linked === null);
					expect(as.recordAs).toBe(linked ?? id);
					// A page's own id goes out as itself only when no guess or
					// pick stands apart from it.
					expect(as.kitsuId !== undefined).toBe(!guess && linked === null);
				}
			)
		);
	});
});
