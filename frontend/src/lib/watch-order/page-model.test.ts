import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import type { WatchOrderData } from './loader';
import { franchisePage } from './franchise-page';
import { captionOf, franchiseLinkOnly, orderOf, settledOrder, watchOrderPage } from './page-model';
import type { HistoryRow } from './progress';
import { stripFor } from './strip-model';
import { franchiseArb, franchiseAroundArb } from './testing/arbitraries';
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

const data = (key: string, anchor: string, rows: HistoryRow[] = []): WatchOrderData => ({
	franchise: payload(key, anchor),
	rows
});

describe('orderOf', () => {
	it('reads the stored order, and anything unknown as linked', () => {
		expect(orderOf('released')).toBe('released');
		expect(orderOf('linked')).toBe('linked');
		for (const v of ['', 'Released', 'story', null, undefined]) expect(orderOf(v)).toBe('linked');
	});
});

describe('settledOrder', () => {
	it('waits for the settings read, then takes its order', () => {
		expect(settledOrder(null, false)).toBeNull();
		expect(settledOrder({ watch_order: 'released' }, false)).toBe('released');
		expect(settledOrder({ watch_order: 'linked' }, false)).toBe('linked');
	});

	it('draws in link order when the settings cannot be read', () => {
		expect(settledOrder(null, true)).toBe('linked');
	});
});

describe('watchOrderPage', () => {
	it('shows no strip when the franchise could not be read', () => {
		expect(watchOrderPage({ franchise: null, rows: [] }, '244', 'linked')).toBeNull();
	});

	it('shows no strip for an entry with no run and no parent story', () => {
		const alone = { ...payload('bleach', '244'), entries: [run('bleach').entries[0]], links: [] };
		expect(watchOrderPage({ franchise: alone, rows: [] }, '244', 'linked')).toBeNull();
	});

	it('places a member of a straight run: "04 of 05" in the caption and the eyebrow', () => {
		const got = watchOrderPage(data('bleach', '48015'), '48015', 'linked')!;
		expect(got.order).toBe('linked');
		expect(got.caption).toEqual({ kind: 'position', position: 4, total: 5 });
		expect(got.eyebrow).toEqual({ position: 4, total: 5 });
		expect(ids(got.view.before)).toEqual(['46903']);
		expect(ids(got.view.after)).toEqual(['49444']);
		expect(got.titles).toEqual(run('bleach').entries.map((e) => e.title));
	});

	it('numbers a loop member nowhere, and says the size of its loop', () => {
		const mono = run('monogatari');
		const looped = mono.entries.find(
			(e) => watchOrderPage(data('monogatari', e.id), e.id, 'linked')?.view.inCycle
		)!;
		const got = watchOrderPage(data('monogatari', looped.id), looped.id, 'linked')!;
		expect(got.caption).toEqual({ kind: 'cycle', count: 10 });
		expect(got.eyebrow).toBeNull();
	});

	it('gives every Monogatari entry its "N of 16" in release order', () => {
		const got = watchOrderPage(data('monogatari', '6546'), '6546', 'released')!;
		expect(got.caption).toEqual({ kind: 'position', position: 2, total: 16 });
		expect(got.eyebrow).toEqual({ position: 2, total: 16 });
	});

	it("names a side story's place, with no number in the eyebrow", () => {
		const got = watchOrderPage(data('fate', '6928'), '6928', 'linked')!;
		expect(got.caption).toEqual({ kind: 'side' });
		expect(got.eyebrow).toBeNull();
		expect(ids(got.view.before)).toEqual(['6028', '7658']);
	});

	it('reads progress from the history, for the run only', () => {
		const rows: HistoryRow[] = [
			{ kitsu_id: '244', ep_no: '366' },
			{ kitsu_id: '43078', ep_no: '4' },
			{ kitsu_id: '99999', ep_no: '2' },
			{ ep_no: '5' }
		];
		const got = watchOrderPage(data('bleach', '48015', rows), '48015', 'linked')!;
		expect(got.progress).toEqual({ '244': 'watched', '43078': 4 });
	});

	it('names the entry being watched elsewhere in the run, which Similar titles leaves out', () => {
		// On Golden Wind, the viewer is on Steel Ball Run, episode 1.
		const rows: HistoryRow[] = [{ kitsu_id: '49847', ep_no: '1' }];
		const got = watchOrderPage(data('jojo', '41410', rows), '41410', 'linked')!;
		expect(got.watching).toMatchObject({ entry: { id: '49847' }, episode: 1 });
		expect(got.similarHidden.has('49847')).toBe(true);
	});

	it('names no entry being watched where a tile already shows it', () => {
		// Steel Ball Run is Stone Ocean Part 3's sequel tile.
		const rows: HistoryRow[] = [{ kitsu_id: '49847', ep_no: '1' }];
		expect(watchOrderPage(data('jojo', '46598', rows), '46598', 'linked')!.watching).toBeNull();
	});

	it('leaves the whole franchise out of Similar titles, not only the tiles', () => {
		// The strip links to the franchise page, which lists all of it.
		const b = run('bleach');
		const got = watchOrderPage(data('bleach', '48015'), '48015', 'linked')!;
		const all = [...b.entries, ...b.versions, ...b.films, ...b.sides, ...b.recaps];
		expect([...got.similarHidden].sort()).toEqual(
			ids(all)
				.filter((x) => x !== '48015')
				.sort()
		);
		expect(got.similarHidden.has('244')).toBe(true);
	});

	it('changes the neighbours with the order where release dates disagree with the links', () => {
		const linked = watchOrderPage(data('fate', '6028'), '6028', 'linked')!;
		const released = watchOrderPage(data('fate', '6028'), '6028', 'released')!;
		expect(ids(linked.view.before)).not.toEqual(ids(released.view.before));
	});

	const anyPage = franchiseArb().chain((f) =>
		fc.record({
			f: fc.constant(f),
			id: fc.constantFrom(f.anchor, ...f.entries.map((e) => e.id)),
			order: fc.constantFrom('linked' as const, 'released' as const),
			rows: fc.array(
				fc.record({
					kitsu_id: fc.option(fc.constantFrom(...f.entries.map((e) => e.id), '0'), {
						nil: undefined
					}),
					ep_no: fc.constantFrom('1', '2', '12', '30', '0.5', 'x')
				}),
				{ maxLength: 6 }
			)
		})
	);

	it('shows a strip exactly where the strip model has one', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const got = watchOrderPage({ franchise: f, rows }, id, order);
				expect(got === null).toBe(stripFor(f, id, order) === null);
			})
		);
	});

	it('never hides the entry itself, and hides the rest of what the franchise page lists', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const got = watchOrderPage({ franchise: f, rows }, id, order);
				if (!got) return;
				expect(got.similarHidden.has(id)).toBe(false);
				const listed = [...f.entries, ...f.versions, ...f.films, ...f.sides, ...f.recaps];
				expect(got.similarHidden).toEqual(new Set(listed.map((e) => e.id).filter((x) => x !== id)));
				for (const e of [...got.view.before, ...got.view.after])
					expect(got.similarHidden.has(e.id)).toBe(true);
				if (got.watching) expect(got.similarHidden.has(got.watching.entry.id)).toBe(true);
			})
		);
	});

	it('puts a position in the eyebrow only with a total, and always in release order when read whole', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const got = watchOrderPage({ franchise: f, rows }, id, order);
				if (!got) return;
				if (got.eyebrow) {
					expect(got.caption).toEqual({ kind: 'position', ...got.eyebrow });
					expect(got.eyebrow.position).toBeGreaterThanOrEqual(1);
					expect(got.eyebrow.position).toBeLessThanOrEqual(got.eyebrow.total);
				}
				if (order === 'released' && got.view.kind === 'run' && f.complete) {
					expect(got.eyebrow).toEqual({
						position: expect.any(Number),
						total: got.run.entries.length
					});
				}
			})
		);
	});

	it('carries the read being partial, and then claims no number anywhere', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const got = watchOrderPage({ franchise: f, rows }, id, order);
				if (!got) return;
				expect(got.complete).toBe(f.complete);
				if (!f.complete) {
					expect(got.caption.kind).not.toBe('position');
					if (got.view.kind === 'run') expect(got.caption).toEqual({ kind: 'partial' });
					expect(got.eyebrow).toBeNull();
				}
			})
		);
	});

	it('names as being watched only another member, part-way, that no tile shows', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const got = watchOrderPage({ franchise: f, rows }, id, order);
				if (!got?.watching) return;
				const w = got.watching.entry.id;
				expect(w).not.toBe(id);
				expect(got.run.entries.some((e) => e.id === w)).toBe(true);
				expect(got.progress[w]).toBe(got.watching.episode);
				expect([...got.view.before, ...got.view.after].some((e) => e.id === w)).toBe(false);
			})
		);
	});
});

describe('franchiseLinkOnly', () => {
	/** Bleach with its first season as the whole run: one main-story entry, films, side stories and recaps. */
	const oneMain = (complete = true): FranchiseData => ({
		...payload('bleach', '244'),
		entries: [run('bleach').entries[0]],
		links: [],
		complete
	});
	const alone = (complete = true): FranchiseData => ({
		...oneMain(complete),
		versions: [],
		films: [],
		sides: [],
		recaps: []
	});

	it('links a show that is its run alone to the franchise page that lists its films and side stories', () => {
		const link = franchiseLinkOnly({ franchise: oneMain(), rows: [] }, '244')!;
		expect(link).not.toBeNull();
		expect(link.complete).toBe(true);
		expect(ids(link.run.entries)).toEqual(['244']);
		const b = run('bleach');
		expect(link.similarHidden).toEqual(
			new Set([...b.films, ...b.sides, ...b.recaps].map((e) => e.id))
		);
	});

	it('links there from a franchise read in part too, carrying that it was', () => {
		const link = franchiseLinkOnly({ franchise: oneMain(false), rows: [] }, '244')!;
		expect(link).not.toBeNull();
		expect(link.complete).toBe(false);
	});

	it('gives no link for a show that is the franchise alone, read whole or in part', () => {
		expect(franchiseLinkOnly({ franchise: alone(), rows: [] }, '244')).toBeNull();
		expect(franchiseLinkOnly({ franchise: alone(false), rows: [] }, '244')).toBeNull();
	});

	it("links a film's page, outside the run, to the franchise page that lists the run", () => {
		const b = run('bleach');
		const film = b.films[0];
		const link = franchiseLinkOnly({ franchise: payload('bleach', film.id), rows: [] }, film.id)!;
		expect(link).not.toBeNull();
		expect(
			watchOrderPage({ franchise: payload('bleach', film.id), rows: [] }, film.id, 'linked')
		).toBeNull();
		expect(link.similarHidden.has(film.id)).toBe(false);
		for (const e of b.entries) expect(link.similarHidden.has(e.id)).toBe(true);
	});

	it('gives no link alone where the strip shows, which carries its own', () => {
		expect(franchiseLinkOnly(data('fate', '6028'), '6028')).toBeNull();
	});

	it('gives no link when the franchise could not be read or answers another show', () => {
		expect(franchiseLinkOnly({ franchise: null, rows: [] }, '244')).toBeNull();
		expect(franchiseLinkOnly({ franchise: oneMain(), rows: [] }, '43078')).toBeNull();
	});

	const anyShow = fc.oneof(franchiseAroundArb(), franchiseArb()).chain((f) =>
		fc.record({
			f: fc.constant(f),
			id: fc.oneof(
				{ weight: 3, arbitrary: fc.constant(f.anchor) },
				{ weight: 1, arbitrary: fc.constantFrom(f.anchor, ...f.entries.map((e) => e.id)) }
			),
			order: fc.constantFrom('linked' as const, 'released' as const)
		})
	);

	it('reaches the franchise page, by the strip or alone, exactly where it lists more than the show', () => {
		fc.assert(
			fc.property(anyShow, ({ f, id, order }) => {
				const d = { franchise: f, rows: [] };
				const strip = watchOrderPage(d, id, order);
				const link = franchiseLinkOnly(d, id);
				expect(strip !== null && link !== null).toBe(false);
				const target = franchisePage(d, id, order);
				const beyond =
					target !== null &&
					[
						...target.run.entries,
						...target.versions,
						...target.films,
						...target.sides,
						...target.recaps
					].some((e) => e.id !== id);
				expect(strip !== null || link !== null).toBe(beyond);
				if (link) {
					expect(link.complete).toBe(f.complete);
					expect(link.similarHidden.has(id)).toBe(false);
					const listed = [...f.entries, ...f.versions, ...f.films, ...f.sides, ...f.recaps];
					expect(link.similarHidden).toEqual(
						new Set(listed.map((e) => e.id).filter((x) => x !== id))
					);
				}
			}),
			{ numRuns: 1000 }
		);
	});
});

describe('captionOf', () => {
	const view = {
		kind: 'run' as const,
		position: null,
		total: null,
		before: [],
		after: [],
		inCycle: false
	};

	it('reads a numbered place, with or without a total', () => {
		expect(captionOf({ ...view, position: 3, total: 9 }, 0, true)).toEqual({
			kind: 'position',
			position: 3,
			total: 9
		});
		expect(captionOf({ ...view, position: 3 }, 0, true)).toEqual({ kind: 'numeral', position: 3 });
	});

	it('reads no number for a franchise read in part: entries may be missing on either side', () => {
		expect(captionOf({ ...view, position: 3, total: 9 }, 0, false)).toEqual({ kind: 'partial' });
		expect(captionOf({ ...view, position: 3 }, 0, false)).toEqual({ kind: 'partial' });
		// A cut walk can leave the show off any fork it really sits on, and
		// leave a loop's count short.
		expect(captionOf(view, 0, false)).toEqual({ kind: 'partial' });
		expect(captionOf({ ...view, inCycle: true }, 10, false)).toEqual({ kind: 'partial' });
		expect(captionOf({ ...view, kind: 'side' }, 0, false)).toEqual({ kind: 'side' });
	});

	it('reads an unnumbered place as a loop or a branch, and a side story as one', () => {
		expect(captionOf({ ...view, inCycle: true }, 10, true)).toEqual({ kind: 'cycle', count: 10 });
		expect(captionOf(view, 0, true)).toEqual({ kind: 'branch' });
		expect(captionOf({ ...view, kind: 'side' }, 0, true)).toEqual({ kind: 'side' });
	});
});
