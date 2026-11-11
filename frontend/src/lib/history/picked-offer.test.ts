// The player's way to change again a card's show.

import { beforeEach, describe, expect, it } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import type { LinkDeps } from './link-offer';
import { notePick, __resetHiddenRowsForTests } from './guess-slip';
import { offerAgain, pickedOffer, surfaceOffer, surfaceOfferRead } from './picked-offer';

const ref = (id: string) => ({ id, canonical_title: `Show ${id}` }) as KitsuAnimeRef;
const CARD: HistoryEntry = { id: 'hianime:black-clover-949', title: 'Black Clover', ep_no: '1' };

function deps(over: Partial<LinkDeps> = {}): LinkDeps {
	return {
		history: async () => [picked],
		resolve: async (e) => ({ match: e.kitsu_id ? ref(e.kitsu_id) : null, trusted: !!e.kitsu_id }),
		search: async () => [],
		link: async () => ({ kitsu_id: '', linked: [] }),
		group: async (_c, rows) => rows,
		...over
	};
}
const picked = { ...CARD, kitsu_id: '4013' };
const d = deps;

beforeEach(() => __resetHiddenRowsForTests());

describe('pickedOffer', () => {
	it("offers to change again the row a pick saved, while it records the page's show", async () => {
		const offer = await pickedOffer(d(), CARD.id, '4013');
		expect(offer).toMatchObject({ entry: picked, saved: '4013' });
		expect(offer?.current.id).toBe('4013');
	});

	it('offers nothing once the row records another show, or is gone', async () => {
		expect(await pickedOffer(d(), CARD.id, '7158')).toBeNull();
		expect(await pickedOffer(d(), 'hianime:gone-1', '4013')).toBeNull();
	});

	it('offers nothing when the history cannot be read', async () => {
		expect(
			await pickedOffer(
				d({ history: async () => Promise.reject(new Error('io')) }),
				CARD.id,
				'4013'
			)
		).toBeNull();
		expect(
			await pickedOffer(
				d({ resolve: async () => Promise.reject(new Error('kitsu')) }),
				CARD.id,
				'4013'
			)
		).toBeNull();
	});
});

describe('offerAgain', () => {
	it('is made only on the player, for a row picked there', async () => {
		expect(await offerAgain(d(), CARD.id, '4013', true)).toBeNull();
		notePick(CARD.id);
		expect((await offerAgain(d(), CARD.id, '4013', true))?.saved).toBe('4013');
		expect(await offerAgain(d(), CARD.id, '4013', false)).toBeNull();
		expect(await offerAgain(d(), null, '4013', true)).toBeNull();
	});
});

describe('a row the user named a show for', () => {
	// The note of the user's naming outlives the app's restart, so every
	// surface offers to change it again: Home, the show page, the player.
	const named = { ...CARD, kitsu_id: '4013', named: true };

	it("is offered again on every surface, while it records the page's show", async () => {
		const n = d({ history: async () => [named] });
		for (const inPlayer of [true, false]) {
			const offer = await offerAgain(n, CARD.id, '4013', inPlayer);
			expect(offer).toMatchObject({ entry: named, saved: '4013' });
		}
		expect(await offerAgain(n, CARD.id, '7158', false)).toBeNull();
	});

	it('is not, for a row the app recorded from a play with no naming', async () => {
		expect(await offerAgain(d(), CARD.id, '4013', false)).toBeNull();
	});
});

describe('surfaceOffer', () => {
	const GUESSED: HistoryEntry = { ...CARD, kitsu_id: undefined };
	/** A row guessed as 7158, which also records nothing the user named. */
	const guessed = () =>
		d({
			history: async () => [GUESSED],
			resolve: async () => ({ match: ref('7158'), trusted: false })
		});

	it("makes a guess's offer on a surface that may ask about one", async () => {
		const offer = await surfaceOffer(guessed(), CARD.id, '7158', {
			asksGuess: true,
			inPlayer: false
		});
		expect(offer).toMatchObject({ entry: GUESSED, current: { id: '7158' } });
		expect(offer?.saved).toBeUndefined();
	});

	it('asks nothing about a guess on a surface no guess opened, and offers a named line again', async () => {
		expect(
			await surfaceOffer(guessed(), CARD.id, '7158', { asksGuess: false, inPlayer: false })
		).toBeNull();
		const named = d({ history: async () => [{ ...picked, named: true }] });
		expect(
			await surfaceOffer(named, CARD.id, '4013', { asksGuess: false, inPlayer: false })
		).toMatchObject({ saved: '4013' });
	});

	it("asks about a guess its session carries, though the session's own play recorded it", async () => {
		// The guessed play landed on the card's own listing and wrote the
		// guess on the row: the row now records 7158, trusted.
		const recorded = { ...CARD, kitsu_id: '7158' };
		const deps = d({ history: async () => [recorded] });
		// The player of a session a guessed card opened carries the card's
		// verdict; the show page it goes up to does too.
		const player = { asksGuess: true, inPlayer: true, carried: true };
		const showPage = { asksGuess: true, inPlayer: false, carried: true };
		const offer = await surfaceOffer(deps, CARD.id, '7158', player);
		expect(offer).toMatchObject({ entry: recorded, current: { id: '7158' } });
		expect(offer?.saved).toBeUndefined();
		// A surface that judges the row itself (Home) finds no guess.
		expect(
			await surfaceOffer(deps, CARD.id, '7158', { asksGuess: true, inPlayer: false })
		).toBeNull();
		// Nor is a row the user named a show for asked about: it is only
		// offered to change again.
		const named = d({ history: async () => [{ ...recorded, named: true }] });
		expect(await surfaceOffer(named, CARD.id, '7158', showPage)).toMatchObject({ saved: '7158' });
		// And a row recording another show than the one playing asks nothing.
		expect(await surfaceOffer(deps, CARD.id, '4013', player)).toBeNull();
	});

	it('offers a line picked on the player again only on the player', async () => {
		notePick(CARD.id);
		const surface = { asksGuess: false };
		expect(await surfaceOffer(d(), CARD.id, '4013', { ...surface, inPlayer: true })).toMatchObject({
			saved: '4013'
		});
		expect(await surfaceOffer(d(), CARD.id, '4013', { ...surface, inPlayer: false })).toBeNull();
	});
});

// A check whose reads failed knows nothing: Home must be able to ask it
// again, rather than take the card for one no longer guessed.
describe('surfaceOfferRead', () => {
	const surface = { asksGuess: true, inPlayer: false };

	it('says it could not read the history or the match', async () => {
		for (const over of [
			{ history: async () => Promise.reject(new Error('io')) },
			{ resolve: async () => Promise.reject(new Error('kitsu')) }
		]) {
			expect(await surfaceOfferRead(d(over), CARD.id, '4013', surface)).toEqual({
				offer: null,
				unread: true
			});
		}
	});

	it('reads as read when the row simply makes no offer', async () => {
		expect(await surfaceOfferRead(d(), CARD.id, '7158', surface)).toEqual({
			offer: null,
			unread: false
		});
	});

	it('hands back the offer it found', async () => {
		const named = d({ history: async () => [{ ...picked, named: true }] });
		const read = await surfaceOfferRead(named, CARD.id, '4013', surface);
		expect(read.unread).toBe(false);
		expect(read.offer).toMatchObject({ saved: '4013' });
	});
});
