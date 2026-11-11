// Which lines a guessed card's play went out for, and for which show,
// in this run of the app.

import { beforeEach, describe, expect, it } from 'vitest';
import { guessPlayed, notePlayed, __resetGuessPlaysForTests } from './guess-plays';

const CARD = 'hianime:gintama-season-5-1385';

beforeEach(() => __resetGuessPlaysForTests());

describe('guess plays', () => {
	it("holds the show a guessed card's play named for its line", () => {
		expect(guessPlayed(CARD, '12553')).toBe(false);
		// A play that landed elsewhere names its guess apart.
		notePlayed({ guess_row: CARD, guess_id: '12553' });
		expect(guessPlayed(CARD, '12553')).toBe(true);
		expect(guessPlayed(CARD, '818')).toBe(false);
		expect(guessPlayed('hianime:other-1', '12553')).toBe(false);
	});

	it('holds one that landed on the card’s own listing, which names the show as its page', () => {
		notePlayed({ guess_row: CARD, kitsu_id: '12553' });
		expect(guessPlayed(CARD, '12553')).toBe(true);
	});

	it('lets go of it once the show is played any other way', () => {
		notePlayed({ guess_row: CARD, guess_id: '12553' });
		// A play of the show from its own page, or a handoff of one.
		notePlayed({ kitsu_id: '12553' });
		expect(guessPlayed(CARD, '12553')).toBe(false);
	});

	it('keeps it through plays of other shows, and plays that name no show', () => {
		notePlayed({ guess_row: CARD, guess_id: '12553' });
		notePlayed({ kitsu_id: '818' });
		notePlayed({});
		expect(guessPlayed(CARD, '12553')).toBe(true);
	});

	it("follows the line's latest guessed play", () => {
		notePlayed({ guess_row: CARD, guess_id: '12553' });
		notePlayed({ guess_row: CARD, guess_id: '818' });
		expect(guessPlayed(CARD, '12553')).toBe(false);
		expect(guessPlayed(CARD, '818')).toBe(true);
	});
});
