// What a surface finds for a Continue card's line: the facts it reads
// and hands the backend, and how it reads the backend's answer. Which
// line is asked about, offered again or kept is the backend's rule
// (commands/link_verdict.rs, its own tests); the fake below answers as
// it does (frontend/src/test/link-rules-fake.ts), so the cases read as the app.

import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef, LinkVerdict, VerdictRequest } from '$lib/api';
import type { LinkDeps } from './link-offer';
import { notePick, __resetHiddenRowsForTests } from './guess-slip';
import { surfaceOfferRead } from './picked-offer';
import { notePlayed, __resetGuessPlaysForTests } from './guess-plays';
import { verdict } from '../../test/link-rules-fake';

const ref = (id: string) => ({ id, canonical_title: `Show ${id}` }) as KitsuAnimeRef;
const CARD: HistoryEntry = { id: 'hianime:black-clover-949', title: 'Black Clover', ep_no: '1' };
const picked = { ...CARD, kitsu_id: '4013' };

function d(over: Partial<LinkDeps> = {}): LinkDeps {
	return {
		history: async () => [picked],
		resolve: async (e) => ({ match: e.kitsu_id ? ref(e.kitsu_id) : null, trusted: !!e.kitsu_id }),
		candidates: async () => [],
		verdict: async (r) => verdict(r),
		link: async () => ({ kitsu_id: '', linked: [] }),
		group: async (r) => [r.card],
		...over
	};
}

const HOME = { asksGuess: true, inPlayer: false };
const PLAYER = { asksGuess: true, inPlayer: true, carried: true };
const SHOW_PAGE = { asksGuess: true, inPlayer: false, carried: true };

beforeEach(() => {
	__resetHiddenRowsForTests();
	__resetGuessPlaysForTests();
});

/** Deps that note every verdict asked. */
function noting(over: Partial<LinkDeps> = {}): { deps: LinkDeps; asked: VerdictRequest[] } {
	const asked: VerdictRequest[] = [];
	const deps = d({
		verdict: async (r) => {
			asked.push(r);
			return verdict(r);
		},
		...over
	});
	return { deps, asked };
}

describe('what a surface tells the backend about the line', () => {
	it('is the page, the match and its trust, the naming, and its own session', async () => {
		const { deps, asked } = noting({
			history: async () => [{ ...CARD, named: true }],
			resolve: async () => ({ match: ref('7158'), trusted: false })
		});
		await surfaceOfferRead(deps, CARD.id, '7158', HOME);
		expect(asked).toEqual([
			{
				page: '7158',
				show: '7158',
				trusted: false,
				named: true,
				asks_guess: true,
				carried: false,
				picked: false,
				in_player: false
			}
		]);
	});

	it("carries the card's verdict only while the line's guessed play stands", async () => {
		const { deps, asked } = noting();
		await surfaceOfferRead(deps, CARD.id, '4013', PLAYER);
		notePlayed({ guess_row: CARD.id, guess_id: '4013' });
		await surfaceOfferRead(deps, CARD.id, '4013', PLAYER);
		await surfaceOfferRead(deps, CARD.id, '4013', HOME);
		expect(asked.map((r) => r.carried)).toEqual([false, true, false]);
	});

	it('says a line was picked here only on the player', async () => {
		const { deps, asked } = noting();
		notePick(CARD.id);
		await surfaceOfferRead(deps, CARD.id, '4013', PLAYER);
		await surfaceOfferRead(deps, CARD.id, '4013', SHOW_PAGE);
		expect(asked.map((r) => r.picked)).toEqual([true, false]);
	});

	it("asks with the visit's signal", async () => {
		const verdictOf = vi.fn<
			(r: VerdictRequest, o?: { signal?: AbortSignal }) => Promise<LinkVerdict>
		>(async (r) => verdict(r));
		const signal = new AbortController().signal;
		await surfaceOfferRead(d({ verdict: verdictOf }), CARD.id, '4013', HOME, { signal });
		expect(verdictOf.mock.calls[0][1]).toEqual({ signal });
	});

	it('asks nothing for no row, a row gone, or a line matching no show', async () => {
		const verdictOf = vi.fn(async (r: VerdictRequest) => verdict(r));
		const none = { offer: null, unread: false, kept: null };
		expect(await surfaceOfferRead(d({ verdict: verdictOf }), null, '4013', HOME)).toEqual(none);
		expect(
			await surfaceOfferRead(d({ verdict: verdictOf }), 'hianime:gone-1', '4013', HOME)
		).toEqual(none);
		const unmatched = d({ verdict: verdictOf, history: async () => [CARD] });
		expect(await surfaceOfferRead(unmatched, CARD.id, '4013', HOME)).toEqual(none);
		expect(verdictOf).not.toHaveBeenCalled();
	});
});

describe('how a surface reads the answer', () => {
	const answering = (answer: LinkVerdict) => d({ verdict: async () => answer });

	it("makes a guess's offer of the show the line shows", async () => {
		const found = await surfaceOfferRead(
			answering({ offer: 'guess', kept: false }),
			CARD.id,
			'4013',
			HOME
		);
		expect(found).toEqual({
			offer: { entry: picked, current: ref('4013') },
			unread: false,
			kept: null
		});
	});

	it('offers again as the way to change the show the line records', async () => {
		const found = await surfaceOfferRead(
			answering({ offer: 'again', kept: false }),
			CARD.id,
			'4013',
			HOME
		);
		expect(found.offer).toEqual({ entry: picked, current: ref('4013'), saved: '4013' });
	});

	it('offers a kept pick again on the show playing, as the player held it before a remount', async () => {
		// The line records the pick (4013); the old show (7158) plays.
		const shown = vi.fn(async (id: string) => ref(id));
		const deps = d({ show: shown, verdict: async () => ({ offer: 'again', kept: true }) });
		const found = await surfaceOfferRead(deps, CARD.id, '7158', PLAYER);
		expect(found).toEqual({
			offer: { entry: picked, current: ref('7158'), saved: '4013' },
			unread: false,
			kept: ref('4013')
		});
		expect(shown.mock.calls[0][0]).toBe('7158');
	});

	it('makes no offer again on a show playing that cannot be read, and keeps the pick', async () => {
		const deps = d({
			show: async () => Promise.reject(new Error('kitsu')),
			verdict: async () => ({ offer: 'again', kept: true })
		});
		expect(await surfaceOfferRead(deps, CARD.id, '7158', PLAYER)).toEqual({
			offer: null,
			unread: false,
			kept: ref('4013')
		});
	});

	it("hands back the line's kept pick", async () => {
		const found = await surfaceOfferRead(
			answering({ offer: null, kept: true }),
			CARD.id,
			'7158',
			PLAYER
		);
		expect(found).toEqual({ offer: null, unread: false, kept: ref('4013') });
	});

	it('says it could not read the history, the match or the answer', async () => {
		for (const over of [
			{ history: async () => Promise.reject(new Error('io')) },
			{ resolve: async () => Promise.reject(new Error('kitsu')) },
			{ verdict: async () => Promise.reject(new Error('backend')) }
		]) {
			expect(await surfaceOfferRead(d(over), CARD.id, '4013', HOME)).toEqual({
				offer: null,
				unread: true,
				kept: null
			});
		}
	});
});

// The app's flows, against the backend's rule as faked.
describe('the offer a surface finds', () => {
	const GUESSED: HistoryEntry = { ...CARD, kitsu_id: undefined };
	const guessed = () =>
		d({
			history: async () => [GUESSED],
			resolve: async () => ({ match: ref('7158'), trusted: false })
		});

	it("is a guess's on a surface that may ask about one, and none on one that may not", async () => {
		expect((await surfaceOfferRead(guessed(), CARD.id, '7158', HOME)).offer).toEqual({
			entry: GUESSED,
			current: ref('7158')
		});
		const quiet = { asksGuess: false, inPlayer: false };
		expect((await surfaceOfferRead(guessed(), CARD.id, '7158', quiet)).offer).toBeNull();
	});

	it("asks about a guess its session carries, though the session's own play recorded it", async () => {
		const recorded = { ...CARD, kitsu_id: '7158' };
		const deps = d({ history: async () => [recorded] });
		notePlayed({ guess_row: CARD.id, guess_id: '7158' });
		expect((await surfaceOfferRead(deps, CARD.id, '7158', PLAYER)).offer).toEqual({
			entry: recorded,
			current: ref('7158')
		});
		expect((await surfaceOfferRead(deps, CARD.id, '7158', HOME)).offer).toBeNull();
		// Once the show is played from its own page, the URL carries nothing.
		notePlayed({ kitsu_id: '7158' });
		expect((await surfaceOfferRead(deps, CARD.id, '7158', PLAYER)).offer).toBeNull();
	});

	it('offers a named line, or one picked on the player, again', async () => {
		const named = d({ history: async () => [{ ...picked, named: true }] });
		expect((await surfaceOfferRead(named, CARD.id, '4013', HOME)).offer?.saved).toBe('4013');
		expect((await surfaceOfferRead(d(), CARD.id, '4013', HOME)).offer).toBeNull();
		notePick(CARD.id);
		expect((await surfaceOfferRead(d(), CARD.id, '4013', PLAYER)).offer?.saved).toBe('4013');
	});

	it('finds the pick a line named for another show keeps, and on the player offers it again', async () => {
		const named = d({
			history: async () => [{ ...picked, named: true }],
			show: async (id) => ref(id)
		});
		const found = await surfaceOfferRead(named, CARD.id, '7158', PLAYER);
		expect(found.kept).toEqual(ref('4013'));
		expect(found.offer).toMatchObject({ current: ref('7158'), saved: '4013' });
		expect((await surfaceOfferRead(named, CARD.id, '7158', SHOW_PAGE)).offer).toBeNull();
		expect((await surfaceOfferRead(named, CARD.id, '4013', PLAYER)).kept).toBeNull();
	});
});
