import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import type { WatchOrderData } from './loader';
import { runOf } from './strip-model';
import { franchiseArb } from './testing/arbitraries';
import type { FranchiseData } from './types';
import { finaleCard, isFinale, upNextFor, type FinaleSelf } from './up-next';

const TODAY = '2026-11-02';
const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;

/** A fixture run as the backend would serve it for `anchor`. */
function payload(key: string, anchor: string): FranchiseData {
	const r = run(key);
	return {
		anchor,
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
		const fresh = { status: 'finished', episodes: 12 };
		expect(finaleCard(jojo, '46598', 'linked', TODAY, 12, fresh)).not.toBeNull();
		const airing = { status: 'current', episodes: 14 };
		expect(finaleCard(jojo, '46598', 'linked', TODAY, 14, airing)).toBeNull();
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
						status: fc.constantFrom('finished', 'current', null),
						episodes: fc.option(fc.integer({ min: 1, max: 40 }), { nil: null })
					}),
					{ nil: null }
				),
				(franchise, order, episode, self) => {
					const id = franchise.anchor;
					const card = finaleCard({ franchise, rows: [] }, id, order, TODAY, episode, self);
					if (card === null) return;
					const r = runOf(franchise)!;
					const members = new Set(r.entries.map((e) => e.id));
					expect(members.has(id)).toBe(true);
					const own = self ?? r.entries.find((e) => e.id === id)!;
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

	it('shows a card wherever the rule names something at a finished finale', () => {
		fc.assert(
			fc.property(
				franchiseArb(),
				fc.constantFrom('linked' as const, 'released' as const),
				(franchise, order) => {
					const id = franchise.anchor;
					const r = runOf(franchise);
					if (r === null || !r.entries.some((e) => e.id === id)) return;
					const self = { status: 'finished', episodes: 3 };
					const card = finaleCard({ franchise, rows: [] }, id, order, TODAY, 3, self);
					const rule = upNextFor(r, id, order, TODAY);
					expect(card === null).toBe(rule.kind === 'none');
				}
			)
		);
	});
});
