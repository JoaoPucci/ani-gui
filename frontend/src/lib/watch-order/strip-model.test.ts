import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import { placement } from './order';
import { runOf, stripFor } from './strip-model';
import { franchiseArb } from './testing/arbitraries';
import type { FranchiseData, WatchEntry } from './types';

const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;
const ids = (list: readonly WatchEntry[]) => list.map((e) => e.id);

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

describe('runOf', () => {
	it('finds the first entry and keeps the sections', () => {
		const got = runOf(payload('jojo', '46598'))!;
		expect(got.nameId).toBe('7158');
		expect(ids(got.entries)).toEqual(ids(run('jojo').entries));
		expect(got.links).toEqual(run('jojo').links);
		expect(ids(got.versions)).toEqual(ids(run('jojo').versions));
	});

	it('keeps only links and parents that name members, once each, and no self-link', () => {
		const p = payload('bleach', '244');
		const got = runOf({
			...p,
			links: [...p.links, ['244', '244'], ['244', '1'], p.links[0]],
			parents: { '684': ['244', '1'], '739': ['1'] }
		})!;
		expect(got.links).toEqual(p.links);
		expect(got.parents).toEqual({ '684': ['244'] });
	});

	it('is null for an empty run', () => {
		expect(runOf({ ...payload('bleach', '244'), entries: [] })).toBeNull();
	});
});

describe('stripFor', () => {
	it("places a member of a straight run, with the run's length", () => {
		const got = stripFor(payload('bleach', '48015'), '48015', 'linked')!;
		expect(got.view).toMatchObject({ kind: 'run', position: 4, total: 5 });
		expect(ids(got.view.before)).toEqual(['46903']);
		expect(ids(got.view.after)).toEqual(['49444']);
	});

	it('shows both ways on from a fork', () => {
		const got = stripFor(payload('fate', '7658'), '7658', 'linked')!;
		expect(ids(got.view.after)).toEqual(['323', '9718']);
		expect(got.view.total).toBeNull();
	});

	it("shows a side story's parent stories", () => {
		const got = stripFor(payload('fate', '6928'), '6928', 'linked')!;
		expect(got.view.kind).toBe('side');
		expect(ids(got.view.before)).toEqual(['6028', '7658']);
		expect(got.view.after).toEqual([]);
	});

	it('follows the order setting', () => {
		const p = payload('monogatari', '6546');
		expect(stripFor(p, '6546', 'released')!.view).toMatchObject({ position: 2, total: 16 });
		expect(stripFor(p, '6546', 'linked')!.view.total).toBeNull();
	});

	it('shows no strip when the fetch failed', () => {
		expect(stripFor(null, '48015', 'linked')).toBeNull();
	});

	it('shows no strip for a payload that answers another id', () => {
		expect(stripFor(payload('bleach', '244'), '48015', 'linked')).toBeNull();
	});

	it('shows no strip for an entry with no run and no parent story', () => {
		const film = run('bleach').films[0];
		const alone: FranchiseData = {
			...payload('bleach', film.id),
			entries: [film],
			links: [],
			parents: {}
		};
		expect(stripFor(alone, film.id, 'linked')).toBeNull();
		expect(stripFor(alone, film.id, 'released')).toBeNull();
	});

	it('shows no strip for an entry around the run with no parent story', () => {
		const film = run('bleach').films[0];
		expect(stripFor(payload('bleach', film.id), film.id, 'linked')).toBeNull();
	});

	it("places a member as the run's ordering does", () => {
		for (const e of run('gintama').entries) {
			const got = stripFor(payload('gintama', e.id), e.id, 'linked')!;
			expect(got.view).toEqual({ kind: 'run', ...placement(run('gintama'), e.id, 'linked') });
		}
	});

	it('never throws, and shows only members of the run it returns', () => {
		const order = fc.constantFrom('linked' as const, 'released' as const);
		fc.assert(
			fc.property(franchiseArb(), order, (f, o) => {
				const got = stripFor(f, f.anchor, o);
				if (got === null) return;
				const members = new Set(ids(got.run.entries));
				expect(members.has(got.run.nameId)).toBe(true);
				for (const e of [...got.view.before, ...got.view.after]) {
					expect(members.has(e.id)).toBe(true);
				}
				for (const [a, b] of got.run.links) {
					expect(a !== b && members.has(a) && members.has(b)).toBe(true);
				}
				if (got.view.kind === 'run') expect(members.has(f.anchor)).toBe(true);
				else expect(got.view.before.length).toBeGreaterThan(0);
			})
		);
	});

	it('shows no strip for a run that is the anchor alone with no parents', () => {
		fc.assert(
			fc.property(franchiseArb(), (f) => {
				const solo = f.entries.find((e) => e.id === f.anchor);
				fc.pre(solo !== undefined);
				expect(stripFor({ ...f, entries: [solo!], parents: {} }, f.anchor, 'linked')).toBeNull();
			})
		);
	});
});
