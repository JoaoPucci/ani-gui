// Whether a show page offers to link the guessed card it was opened
// from, and the flow that links it.

import { describe, expect, it, vi } from 'vitest';
import type { HistoryEntry, KitsuAnimeRef, LinkOutcome } from '$lib/api';
import {
	confirmLink,
	guessedGroup,
	linkOffer,
	loadCandidates,
	searchCandidates,
	undoLink,
	type LinkDeps
} from './link-offer';

function ref(id: string, over: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Show ${id}`,
		slug: null,
		synopsis: null,
		start_date: null,
		end_date: null,
		episode_count: null,
		average_rating: null,
		subtype: 'TV',
		status: 'finished',
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null,
		...over
	};
}

const CARD: HistoryEntry = { id: 'hianime:black-clover-949', title: 'Black Clover', ep_no: '1' };
const HIDDEN: HistoryEntry = { id: 'black-clover-77', title: 'Black Clover', ep_no: '3' };
const PLAYED: HistoryEntry = {
	id: 'hianime:black-clover-special-3392',
	title: 'Black Clover: Jump Festa 2016 Special',
	ep_no: '1',
	kitsu_id: '12625'
};
const OTHER: HistoryEntry = { id: 'hianime:gintama-1237', title: 'Gintama', ep_no: '175' };
const GUESS = '12625';
const TITLE_GONE = 'Black Clover';

/** Every row guessed as the special, Gintama as itself. */
function deps(over: Partial<LinkDeps> = {}): LinkDeps {
	return {
		history: async () => [CARD, HIDDEN, PLAYED, OTHER],
		resolve: async (e) =>
			e.kitsu_id
				? { match: ref(e.kitsu_id), trusted: true }
				: e.id === OTHER.id
					? { match: ref('818'), trusted: false }
					: { match: ref(GUESS), trusted: false },
		search: async () => [],
		link: async (rows, kitsuId) => ({
			kitsu_id: kitsuId,
			linked: rows.map((row) => ({ row, previous: null, ep_no: '1', watched_at: null }))
		}),
		unlink: async (_k, rows) => ({ restored: rows.map((r) => r.row), kept: [] }),
		...over
	};
}

describe('linkOffer', () => {
	it('offers the guessed card row the page was opened from', async () => {
		const offer = await linkOffer(deps(), CARD.id, GUESS);
		expect(offer?.entry).toEqual(CARD);
		expect(offer?.current.id).toBe(GUESS);
	});

	it('offers nothing for a page opened without a card', async () => {
		expect(await linkOffer(deps(), null, GUESS)).toBeNull();
	});

	it('offers nothing once the row records an id, or is gone', async () => {
		expect(await linkOffer(deps(), PLAYED.id, GUESS)).toBeNull();
		expect(await linkOffer(deps(), 'hianime:removed-1', GUESS)).toBeNull();
	});

	it('offers nothing when the row is not guessed as this show', async () => {
		expect(await linkOffer(deps(), CARD.id, '13209')).toBeNull();
		const trusted = deps({ resolve: async () => ({ match: ref(GUESS), trusted: true }) });
		expect(await linkOffer(trusted, CARD.id, GUESS)).toBeNull();
	});

	it('offers nothing when the history cannot be read', async () => {
		const failing = deps({ history: async () => Promise.reject(new Error('down')) });
		expect(await linkOffer(failing, CARD.id, GUESS)).toBeNull();
	});
});

describe('a row whose recorded show Kitsu no longer serves', () => {
	// Continue Watching matches such a row as one with no id, by a
	// guess, and its card opens a guessed session.
	const GONE: HistoryEntry = {
		id: 'hianime:black-clover-1',
		title: TITLE_GONE,
		ep_no: '2',
		kitsu_id: '404'
	};
	const fallback = (over: Partial<LinkDeps> = {}) =>
		deps({
			history: async () => [CARD, GONE],
			resolve: async () => ({ match: ref(GUESS), trusted: false }),
			...over
		});

	it('is offered like any guessed row', async () => {
		expect((await linkOffer(fallback(), GONE.id, GUESS))?.entry).toEqual(GONE);
	});

	it('is linked with the card it hides behind', async () => {
		const offer = (await linkOffer(fallback(), CARD.id, GUESS))!;
		expect(await guessedGroup(fallback(), offer)).toEqual([CARD.id, GONE.id]);
	});
});

describe('loadCandidates', () => {
	it('searches the saved title and the slug words, the guess first', async () => {
		const search = vi.fn(async (q: string) =>
			q === 'Black Clover' ? [ref('13209'), ref(GUESS)] : [ref('44242')]
		);
		const offer = (await linkOffer(deps(), CARD.id, GUESS))!;
		const out = await loadCandidates(deps({ search }), offer);
		expect(search.mock.calls.map((c) => c[0]).sort()).toEqual(['Black Clover', 'black clover']);
		expect(out.map((c) => c.ref.id)).toEqual([GUESS, '13209', '44242']);
	});

	it('keeps what one search found when the other fails', async () => {
		const search = vi.fn(async (q: string) => {
			if (q === 'Black Clover') throw new Error('down');
			return [ref('13209')];
		});
		const offer = (await linkOffer(deps(), CARD.id, GUESS))!;
		const out = await loadCandidates(deps({ search }), offer);
		expect(out.map((c) => c.ref.id)).toEqual([GUESS, '13209']);
	});

	it('searches only the title for an id with no slug words', async () => {
		const search = vi.fn<(q: string) => Promise<KitsuAnimeRef[]>>(async () => []);
		const offer = {
			entry: { id: 'vDTSJHSpYnrkZnAvG', title: 'Old', ep_no: '1' },
			current: ref(GUESS)
		};
		await loadCandidates(deps({ search }), offer);
		expect(search.mock.calls.map((c) => c[0])).toEqual(['Old']);
	});
});

describe('searchCandidates', () => {
	it('answers the typed search, marking the show shown now', async () => {
		const search = vi.fn(async () => [ref('7730'), ref(GUESS)]);
		const offer = (await linkOffer(deps(), CARD.id, GUESS))!;
		const out = await searchCandidates(deps({ search }), offer, '  jojo 2000 ');
		expect(search).toHaveBeenCalledWith('jojo 2000');
		expect(out.map((c) => [c.ref.id, c.current])).toEqual([
			['7730', false],
			[GUESS, true]
		]);
	});
});

describe('guessedGroup', () => {
	it('is the card row and every other unrecorded row guessed as the same show', async () => {
		const offer = (await linkOffer(deps(), CARD.id, GUESS))!;
		expect(await guessedGroup(deps(), offer)).toEqual([CARD.id, HIDDEN.id]);
	});

	it('leaves out a row whose match cannot be read', async () => {
		const flaky = deps({
			resolve: async (e) => {
				if (e.id === HIDDEN.id) throw new Error('down');
				if (e.kitsu_id) return { match: ref(e.kitsu_id), trusted: true };
				return { match: ref(GUESS), trusted: false };
			}
		});
		const offer = { entry: CARD, current: ref(GUESS) };
		expect(await guessedGroup(flaky, offer)).toEqual([CARD.id, OTHER.id]);
	});
});

describe('guessedGroup, over a long history', () => {
	it('asks for the rows’ matches side by side', async () => {
		let inFlight = 0;
		let most = 0;
		const many: HistoryEntry[] = Array.from({ length: 6 }, (_, i) => ({
			id: `row-${i}`,
			title: 'Black Clover',
			ep_no: '1'
		}));
		const slow = deps({
			history: async () => [CARD, ...many],
			resolve: async () => {
				inFlight++;
				most = Math.max(most, inFlight);
				await new Promise((r) => setTimeout(r, 5));
				inFlight--;
				return { match: ref(GUESS), trusted: false };
			}
		});
		const offer = { entry: CARD, current: ref(GUESS) };
		expect(await guessedGroup(slow, offer)).toEqual([CARD.id, ...many.map((r) => r.id)]);
		expect(most).toBeGreaterThan(1);
	});
});

describe('confirmLink and undoLink', () => {
	it('links the whole group to the chosen show', async () => {
		const link = vi.fn(deps().link);
		const offer = (await linkOffer(deps(), CARD.id, GUESS))!;
		const outcome = await confirmLink(deps({ link }), offer, '13209');
		expect(link).toHaveBeenCalledWith([CARD.id, HIDDEN.id], '13209');
		expect(outcome.kitsu_id).toBe('13209');
	});

	it('says whether the undo gave every row back', async () => {
		const outcome: LinkOutcome = {
			kitsu_id: '13209',
			linked: [{ row: CARD.id, previous: null, ep_no: '1', watched_at: null }]
		};
		const unlink = vi.fn(deps().unlink);
		expect(await undoLink(deps({ unlink }), outcome)).toBe(true);
		expect(unlink).toHaveBeenCalledWith('13209', outcome.linked);
		const kept = deps({ unlink: async () => ({ restored: [], kept: [CARD.id] }) });
		expect(await undoLink(kept, outcome)).toBe(false);
	});
});
