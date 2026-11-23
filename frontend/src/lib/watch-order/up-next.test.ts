import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import type { WatchOrderData } from './loader';
import { runOf } from './strip-model';
import { franchiseArb } from './testing/arbitraries';
import type { FranchiseData } from './types';
import { chainEndCard, playerUpNext } from './chain-end';
import { finaleCard, isFinale, upNextFor, type FinaleSelf } from './up-next';

const TODAY = '2026-11-02';
const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;

/** A fixture run as the backend would serve it for `anchor`. */
function payload(key: string, anchor: string): FranchiseData {
	const r = run(key);
	return {
		anchor,
		name: `The ${key} franchise`,
		name_from: r.nameId,
		entries: r.entries,
		links: r.links,
		versions: r.versions,
		films: r.films,
		sides: r.sides,
		recaps: r.recaps,
		parents: r.parents[anchor] ? { [anchor]: r.parents[anchor] } : {},
		complete: true
	};
}

const data = (key: string, anchor: string, rows: WatchOrderData['rows'] = []): WatchOrderData => ({
	franchise: payload(key, anchor),
	rows
});

const ids = (card: ReturnType<typeof finaleCard>) =>
	card === null
		? null
		: card.upNext.kind === 'choices'
			? card.upNext.entries.map((e) => e.id)
			: [card.upNext.entry.id];

describe('upNextFor', () => {
	it('answers the end card in each of its four cases', () => {
		expect(upNextFor(run('bleach'), '43078', 'linked', TODAY)).toMatchObject({
			kind: 'one',
			entry: { id: '46903' }
		});
		expect(upNextFor(run('monogatari'), '48434', 'linked', TODAY)).toMatchObject({
			kind: 'unaired',
			entry: { id: '50013' }
		});
		expect(upNextFor(run('fate'), '7658', 'linked', TODAY).kind).toBe('choices');
		expect(upNextFor(run('bleach'), '49444', 'linked', TODAY).kind).toBe('none');
	});
});

describe('isFinale', () => {
	const finished = (episodes: number | null): FinaleSelf => ({ status: 'finished', episodes });

	it('is the last episode of an entry that has finished airing', () => {
		expect(isFinale(finished(12), 12)).toBe(true);
		expect(isFinale(finished(12), 11)).toBe(false);
		expect(isFinale(finished(1), 1)).toBe(true);
	});

	it('counts an episode past the listed count, or an extra after the last, as the finale', () => {
		expect(isFinale(finished(12), 13)).toBe(true);
		expect(isFinale(finished(12), 12.5)).toBe(true);
		expect(isFinale(finished(12), 11.5)).toBe(false);
	});

	it('has no finale while the entry airs, or with no episode count', () => {
		for (const status of ['current', 'upcoming', 'tba', 'unreleased', null])
			expect(isFinale({ status, episodes: 12 }, 12)).toBe(false);
		expect(isFinale(finished(null), 12)).toBe(false);
	});

	it('holds from the last episode on, and only for a finished entry with a count', () => {
		fc.assert(
			fc.property(
				fc.constantFrom('finished', 'current', 'tba', null),
				fc.option(fc.integer({ min: 1, max: 400 }), { nil: null }),
				fc.double({ min: 0, max: 500, noNaN: true }),
				fc.double({ min: 0, max: 50, noNaN: true }),
				(status, episodes, episode, later) => {
					const self = { status, episodes };
					const got = isFinale(self, episode);
					expect(got).toBe(status === 'finished' && episodes !== null && episode >= episodes);
					if (got) expect(isFinale(self, episode + later)).toBe(true);
				}
			)
		);
	});
});

describe('finaleCard', () => {
	it('names the next entry at a finale', () => {
		const card = finaleCard(data('bleach', '43078'), '43078', 'linked', TODAY, 13, null);
		expect(card?.upNext).toMatchObject({ kind: 'one', entry: { id: '46903' } });
	});

	it('shows no card before the finale, or after the last entry', () => {
		expect(finaleCard(data('bleach', '43078'), '43078', 'linked', TODAY, 12, null)).toBeNull();
		expect(finaleCard(data('bleach', '49444'), '49444', 'linked', TODAY, 10, null)).toBeNull();
	});

	it('waits for the franchise and the settings, and ignores a franchise read for another id', () => {
		expect(finaleCard(null, '43078', 'linked', TODAY, 13, null)).toBeNull();
		expect(
			finaleCard({ franchise: null, rows: [] }, '43078', 'linked', TODAY, 13, null)
		).toBeNull();
		expect(finaleCard(data('bleach', '43078'), '43078', null, TODAY, 13, null)).toBeNull();
		expect(finaleCard(data('bleach', '244'), '43078', 'linked', TODAY, 13, null)).toBeNull();
	});

	it('shows no card for a franchise read only in part: the next entry may be missing', () => {
		const d = data('bleach', '43078');
		const part = { ...d, franchise: { ...d.franchise!, complete: false } };
		expect(finaleCard(part, '43078', 'linked', TODAY, 13, null)).toBeNull();
		expect(finaleCard(part, '43078', 'released', TODAY, 13, null)).toBeNull();
	});

	it('shows no card for a side story outside the run', () => {
		expect(finaleCard(data('fate', '6928'), '6928', 'linked', TODAY, 1, null)).toBeNull();
		expect(finaleCard(data('fate', '6928'), '6928', 'released', TODAY, 1, null)).toBeNull();
	});

	it('follows the order setting', () => {
		// Fate/Zero 2nd Season forks in link order; released, the prologue alone follows.
		const linked = finaleCard(data('fate', '7658'), '7658', 'linked', TODAY, 12, null);
		expect(linked?.upNext.kind).toBe('choices');
		expect(ids(linked)).toEqual(['323', '9718']);
		const released = finaleCard(data('fate', '7658'), '7658', 'released', TODAY, 12, null);
		expect(released?.upNext).toMatchObject({ kind: 'one', entry: { id: '9718' } });
		// Fate/stay night (2006) ends its branch when linked, and leads to Fate/Zero when released.
		expect(finaleCard(data('fate', '323'), '323', 'linked', TODAY, 24, null)).toBeNull();
		expect(ids(finaleCard(data('fate', '323'), '323', 'released', TODAY, 24, null))).toEqual([
			'6028'
		]);
	});

	it('names a next entry that has not aired, as information', () => {
		const card = finaleCard(data('monogatari', '48434'), '48434', 'linked', TODAY, 14, null);
		expect(card?.upNext).toMatchObject({ kind: 'unaired', entry: { id: '50013' } });
	});

	it("decides the finale by the player's own reading of the entry when it has one", () => {
		// The run lists Stone Ocean Part 3 at 14 episodes, finished.
		const jojo = data('jojo', '46598');
		expect(finaleCard(jojo, '46598', 'linked', TODAY, 14, null)).not.toBeNull();
		expect(finaleCard(jojo, '46598', 'linked', TODAY, 12, null)).toBeNull();
		// The player's reading wins where the two differ.
		const fresh = { id: '46598', status: 'finished', episodes: 12 };
		expect(finaleCard(jojo, '46598', 'linked', TODAY, 12, fresh)).not.toBeNull();
		const airing = { id: '46598', status: 'current', episodes: 14 };
		expect(finaleCard(jojo, '46598', 'linked', TODAY, 14, airing)).toBeNull();
	});

	it("ignores the player's reading of another show", () => {
		// The page's detail still belongs to the show it was opened on
		// while the franchise is already the new show's: that reading
		// says nothing about this entry, and the run's entry decides.
		const jojo = data('jojo', '46598');
		const other = { id: '43078', status: 'current', episodes: 14 };
		expect(finaleCard(jojo, '46598', 'linked', TODAY, 14, other)).not.toBeNull();
		const otherFinished = { id: '43078', status: 'finished', episodes: 12 };
		expect(finaleCard(jojo, '46598', 'linked', TODAY, 12, otherFinished)).toBeNull();
	});

	it("carries the next entries' progress from the history", () => {
		const rows = [
			{ kitsu_id: '46903', ep_no: '4' },
			{ kitsu_id: '43078', ep_no: '13' }
		];
		const card = finaleCard(data('bleach', '43078', rows), '43078', 'linked', TODAY, 13, null);
		expect(card?.progress['46903']).toBe(4);
	});

	it('never throws, and names only run members after the entry, as the rule does', () => {
		fc.assert(
			fc.property(
				franchiseArb(),
				fc.constantFrom('linked' as const, 'released' as const),
				fc.integer({ min: 1, max: 40 }),
				fc.option(
					fc.record({
						// The player's reading of this show, or of the show
						// the page was opened on.
						ours: fc.boolean(),
						status: fc.constantFrom('finished', 'current', null),
						episodes: fc.option(fc.integer({ min: 1, max: 40 }), { nil: null })
					}),
					{ nil: null }
				),
				(franchise, order, episode, reading) => {
					const id = franchise.anchor;
					const self = reading && {
						id: reading.ours ? id : `${id}-other`,
						status: reading.status,
						episodes: reading.episodes
					};
					const card = finaleCard({ franchise, rows: [] }, id, order, TODAY, episode, self);
					if (card === null) return;
					const r = runOf(franchise)!;
					const members = new Set(r.entries.map((e) => e.id));
					expect(members.has(id)).toBe(true);
					const own = self?.id === id ? self : r.entries.find((e) => e.id === id)!;
					expect(isFinale(own, episode)).toBe(true);
					for (const n of ids(card)!) {
						expect(members.has(n)).toBe(true);
						expect(n).not.toBe(id);
					}
					expect(card.upNext).toEqual(upNextFor(r, id, order, TODAY));
				}
			)
		);
	});

	it('shows a card wherever the rule names something at a finished finale of a franchise read whole', () => {
		fc.assert(
			fc.property(
				franchiseArb(),
				fc.constantFrom('linked' as const, 'released' as const),
				(franchise, order) => {
					const id = franchise.anchor;
					const r = runOf(franchise);
					if (r === null || !r.entries.some((e) => e.id === id)) return;
					const self = { id, status: 'finished', episodes: 3 };
					const card = finaleCard({ franchise, rows: [] }, id, order, TODAY, 3, self);
					const rule = upNextFor(r, id, order, TODAY);
					expect(card === null).toBe(rule.kind === 'none' || !franchise.complete);
				}
			)
		);
	});
});

describe('playerUpNext', () => {
	it('is the finale card where there is one, else the chain-end card, else none', () => {
		fc.assert(
			fc.property(
				franchiseArb(),
				fc.constantFrom('linked' as const, 'released' as const),
				fc.integer({ min: 1, max: 40 }),
				(franchise: FranchiseData, order, episode) => {
					const data: WatchOrderData = { franchise, rows: [] };
					const id = franchise.anchor;
					const finale = finaleCard(data, id, order, TODAY, episode, null);
					const end = chainEndCard(data, id, order, TODAY, episode, null);
					const shown = playerUpNext(data, id, order, TODAY, episode, null);
					if (finale) expect(shown).toEqual({ kind: 'entry', card: finale });
					else if (end) expect(shown).toEqual({ kind: 'franchise', card: end });
					else expect(shown).toBeNull();
				}
			)
		);
	});

	it("reads another show's reading as no reading at all", () => {
		fc.assert(
			fc.property(
				franchiseArb(),
				fc.constantFrom('linked' as const, 'released' as const),
				fc.integer({ min: 1, max: 40 }),
				fc.constantFrom('finished', 'current', null),
				fc.option(fc.integer({ min: 1, max: 40 }), { nil: null }),
				(franchise: FranchiseData, order, episode, status, episodes) => {
					const data: WatchOrderData = { franchise, rows: [] };
					const id = franchise.anchor;
					const other = { id: `${id}-other`, status, episodes };
					expect(finaleCard(data, id, order, TODAY, episode, other)).toEqual(
						finaleCard(data, id, order, TODAY, episode, null)
					);
					expect(chainEndCard(data, id, order, TODAY, episode, other)).toEqual(
						chainEndCard(data, id, order, TODAY, episode, null)
					);
					expect(playerUpNext(data, id, order, TODAY, episode, other)).toEqual(
						playerUpNext(data, id, order, TODAY, episode, null)
					);
				}
			)
		);
	});
});

describe('finaleCard on an entry outside the main story', () => {
	const e = (id: string, start: string, subtype = 'movie') => ({
		id,
		title: `T${id}`,
		subtype,
		start,
		episodes: 1,
		status: 'finished',
		poster: null,
		cover: null
	});
	const main = [e('1', '2000-01-01', 'TV'), e('2', '2004-01-01', 'TV')];
	const f1 = e('10', '2001-01-01');
	const f2 = e('11', '2002-01-01');
	const chainData = (anchor: string, into: boolean): WatchOrderData => ({
		franchise: {
			anchor,
			name: 'The franchise',
			name_from: '1',
			entries: main,
			links: [['1', '2']],
			versions: [],
			films: [f1, f2],
			sides: [],
			recaps: [],
			parents: {},
			complete: true,
			chain: {
				entries: into ? [f1, f2, main[1]] : [f1, f2],
				links: into
					? [
							['10', '11'],
							['11', '2']
						]
					: [['10', '11']],
				seams: into ? ['2'] : []
			}
		},
		rows: []
	});

	it("names the chain's next title", () => {
		expect(ids(finaleCard(chainData('10', false), '10', 'linked', TODAY, 1, null))).toEqual(['11']);
	});

	it('goes on into the main story at the end of a chain that leads into it', () => {
		expect(ids(finaleCard(chainData('11', true), '11', 'linked', TODAY, 1, null))).toEqual(['2']);
	});

	it('names nothing at the end of a chain that leads nowhere', () => {
		expect(finaleCard(chainData('11', false), '11', 'linked', TODAY, 1, null)).toBeNull();
	});

	it('offers the franchise page at the end of a chain that leads nowhere', () => {
		const end = chainEndCard(chainData('11', false), '11', 'linked', TODAY, 1, null)!;
		expect(end.name).toBe('The franchise');
		expect(end.id).toBe('11');
		// The strip link's stack: the main story's two titles, the films.
		expect(end.summary.main).toBe(2);
		expect(end.summary.films).toBe(2);
	});

	it('offers no franchise card where the card names a next title, or short of the finale', () => {
		expect(chainEndCard(chainData('10', false), '10', 'linked', TODAY, 1, null)).toBeNull();
		expect(chainEndCard(chainData('11', true), '11', 'linked', TODAY, 1, null)).toBeNull();
		const unfinished = { id: '11', status: 'current', episodes: 1 };
		expect(chainEndCard(chainData('11', false), '11', 'linked', TODAY, 1, unfinished)).toBeNull();
	});

	it('offers no franchise card in a franchise read in part, or on a main-story page', () => {
		const partial = chainData('11', false);
		const cut = { ...partial, franchise: { ...partial.franchise!, complete: false } };
		expect(chainEndCard(cut, '11', 'linked', TODAY, 1, null)).toBeNull();
		const main = {
			...partial,
			franchise: { ...partial.franchise!, anchor: '2', chain: undefined }
		};
		expect(chainEndCard(main, '2', 'linked', TODAY, 1, null)).toBeNull();
		expect(chainEndCard(partial, '11', null, TODAY, 1, null)).toBeNull();
		expect(chainEndCard(null, '11', 'linked', TODAY, 1, null)).toBeNull();
	});

	it("ignores the player's reading of another show", () => {
		// A page whose id moved to another show still holds the first
		// one's detail: an airing reading of it does not hide this
		// finale's card.
		const other = { id: '10', status: 'current', episodes: 1 };
		expect(chainEndCard(chainData('11', false), '11', 'linked', TODAY, 1, other)).not.toBeNull();
		expect(playerUpNext(chainData('11', false), '11', 'linked', TODAY, 1, other)?.kind).toBe(
			'franchise'
		);
	});

	it('fronts the franchise card with the entry the name comes from', () => {
		const end = chainEndCard(chainData('11', false), '11', 'linked', TODAY, 1, null)!;
		expect(end.front).toBe('1');
	});

	it('fronts the franchise card as its page does when the name comes from no main-story title', () => {
		const data = chainData('11', false);
		const off = { ...data, franchise: { ...data.franchise!, name_from: '99' } };
		const end = chainEndCard(off, '11', 'linked', TODAY, 1, null)!;
		// The franchise page takes the main story's first title then.
		expect(end.front).toBe('1');
	});

	it('shows one card: the next title where the finale names one, else the franchise', () => {
		const next = playerUpNext(chainData('10', false), '10', 'linked', TODAY, 1, null);
		expect(next?.kind).toBe('entry');
		const end = playerUpNext(chainData('11', false), '11', 'linked', TODAY, 1, null);
		expect(end?.kind).toBe('franchise');
		expect(end?.kind === 'franchise' && end.card.name).toBe('The franchise');
		const unfinished = { id: '11', status: 'current', episodes: 1 };
		expect(playerUpNext(chainData('11', false), '11', 'linked', TODAY, 1, unfinished)).toBeNull();
	});
});
