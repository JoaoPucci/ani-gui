// The player's way to change again a card's show.

import { beforeEach, describe, expect, it } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import type { LinkDeps } from './link-offer';
import { notePick, __resetHiddenRowsForTests } from './guess-slip';
import { offerAgain, pickedOffer, surfaceOffer } from './picked-offer';

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

	it('offers a line picked on the player again only on the player', async () => {
		notePick(CARD.id);
		const surface = { asksGuess: false };
		expect(await surfaceOffer(d(), CARD.id, '4013', { ...surface, inPlayer: true })).toMatchObject({
			saved: '4013'
		});
		expect(await surfaceOffer(d(), CARD.id, '4013', { ...surface, inPlayer: false })).toBeNull();
	});
});
