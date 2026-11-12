// The picker's reads for a guessed card's line: what it asks the
// backend for its candidates and for the rows a link covers, and how it
// reads the answers. The candidates' order and warnings, and which rows
// a correction covers, are the backend's rules (commands/link_rules.rs,
// link_verdict.rs, their own tests).

import { describe, expect, it, vi } from 'vitest';
import type {
	CandidatesRequest,
	HistoryEntry,
	KitsuAnimeRef,
	LinkCandidate,
	LinkGroupRequest
} from '$lib/api';
import {
	confirmLink,
	guessedGroup,
	loadCandidates,
	searchCandidates,
	type LinkDeps,
	type LinkOffer
} from './link-offer';
import { groupMembers } from '../../test/link-rules-fake';

const ref = (id: string) => ({ id, canonical_title: `Show ${id}` }) as KitsuAnimeRef;

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
const OFFER: LinkOffer = { entry: CARD, current: ref(GUESS) };
const FOUND: LinkCandidate[] = [{ ref: ref(GUESS), current: true, warnings: ['episode'] }];

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
		candidates: async () => FOUND,
		verdict: async () => ({ offer: null, kept: false }),
		link: async (_card, rows, kitsuId) => ({
			kitsu_id: kitsuId,
			linked: rows.map((row) => ({ row, previous: null, ep_no: '1', watched_at: null }))
		}),
		// As the backend chooses them, every row sharing the card's listing.
		group: async (r) => groupMembers(r.card, r.show, r.saved, r.rows),
		...over
	};
}

describe('loadCandidates', () => {
	it("asks for the row's candidates by its saved title, the card's show and the episode offered", async () => {
		const candidates = vi.fn<(r: CandidatesRequest) => Promise<LinkCandidate[]>>(async () => FOUND);
		const legacy = { ...OFFER, entry: { ...CARD, title: 'Black Clover (170 episodes)' } };
		const out = await loadCandidates(deps({ candidates }), legacy, 4);
		expect(candidates.mock.calls[0][0]).toEqual({
			row: CARD.id,
			// As Continue Watching searches it.
			title: 'Black Clover',
			current: ref(GUESS),
			query: null,
			episode: 4
		});
		expect(out).toEqual({ candidates: FOUND, failed: false });
	});

	it("says so when the backend cannot answer, keeping the card's own show to pick", async () => {
		const down = deps({ candidates: async () => Promise.reject(new Error('down')) });
		expect(await loadCandidates(down, OFFER, 1)).toEqual({
			candidates: [{ ref: ref(GUESS), current: true, warnings: [] }],
			failed: true
		});
	});

	it("asks with the arrival's signal", async () => {
		const candidates = vi.fn<
			(r: CandidatesRequest, o?: { signal?: AbortSignal }) => Promise<LinkCandidate[]>
		>(async () => FOUND);
		const signal = new AbortController().signal;
		await loadCandidates(deps({ candidates }), OFFER, 1, { signal });
		expect(candidates.mock.calls[0][1]).toEqual({ signal });
	});
});

describe('searchCandidates', () => {
	it('asks for the typed words, and lets their failure through', async () => {
		const candidates = vi.fn<(r: CandidatesRequest) => Promise<LinkCandidate[]>>(async () => FOUND);
		expect(await searchCandidates(deps({ candidates }), OFFER, '  jojo 2000 ', 2)).toEqual(FOUND);
		expect(candidates.mock.calls[0][0]).toMatchObject({ query: '  jojo 2000 ', episode: 2 });
		const down = deps({ candidates: async () => Promise.reject(new Error('down')) });
		await expect(searchCandidates(down, OFFER, 'jojo', 2)).rejects.toThrow('down');
	});
});

describe('guessedGroup', () => {
	it("hands the backend every other row with its match, and answers the backend's rows", async () => {
		const group = vi.fn<(r: LinkGroupRequest) => Promise<string[]>>(async () => [
			CARD.id,
			HIDDEN.id
		]);
		expect(await guessedGroup(deps({ group }), OFFER)).toEqual([CARD.id, HIDDEN.id]);
		expect(group).toHaveBeenCalledWith({
			card: CARD.id,
			show: GUESS,
			saved: null,
			rows: [
				{ id: HIDDEN.id, show: GUESS, trusted: false },
				{ id: PLAYED.id, show: GUESS, trusted: true },
				{ id: OTHER.id, show: '818', trusted: false }
			]
		});
	});

	it('names no show for a row whose match cannot be read', async () => {
		const group = vi.fn(async (r: LinkGroupRequest) =>
			groupMembers(r.card, r.show, r.saved, r.rows)
		);
		const flaky = deps({
			group,
			resolve: async (e) => {
				if (e.id === HIDDEN.id) throw new Error('down');
				return { match: ref(GUESS), trusted: false };
			}
		});
		expect(await guessedGroup(flaky, OFFER)).toEqual([CARD.id, PLAYED.id, OTHER.id]);
		expect(group.mock.calls[0][0].rows[0]).toEqual({ id: HIDDEN.id, show: null, trusted: false });
	});

	it("names the show the user named for the card's line", async () => {
		const group = vi.fn(async (r: LinkGroupRequest) =>
			groupMembers(r.card, r.show, r.saved, r.rows)
		);
		const picked = (e: HistoryEntry) => ({ ...e, kitsu_id: '4013' });
		const d = deps({ group, history: async () => [picked(CARD), picked(HIDDEN), PLAYED, OTHER] });
		const offer = { entry: picked(CARD), current: ref(GUESS), saved: '4013' };
		expect(await guessedGroup(d, offer)).toEqual([CARD.id, HIDDEN.id]);
		expect(group.mock.calls[0][0].saved).toBe('4013');
	});

	it('matches the rows side by side', async () => {
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
		expect(await guessedGroup(slow, OFFER)).toEqual([CARD.id, ...many.map((r) => r.id)]);
		expect(most).toBeGreaterThan(1);
	});
});

describe('confirmLink', () => {
	it('links the rows the backend says the card stands for to the chosen show', async () => {
		const link = vi.fn(deps().link);
		const outcome = await confirmLink(deps({ link }), OFFER, '13209');
		expect(link).toHaveBeenCalledWith(CARD.id, [CARD.id, HIDDEN.id], '13209');
		expect(outcome.kitsu_id).toBe('13209');
	});
});
