import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { FIXTURE_RUNS, type FixtureRun } from './fixtures/runs';
import {
	franchisePage,
	franchiseTrail,
	type FranchiseRow,
	type FranchisePageModel
} from './franchise-page';
import { withoutFormatSuffix } from './franchise-name';
import type { WatchOrderData } from './loader';
import { pathNote, placement, segments, sequence } from './order';
import type { HistoryRow } from './progress';
import { runOf } from './strip-model';
import { currentIn } from './strip-view';
import { franchiseArb, franchiseAroundArb } from './testing/arbitraries';
import type { FranchiseData } from './types';

const run = (key: string): FixtureRun => FIXTURE_RUNS.find((r) => r.key === key)!;
const ids = (list: readonly { readonly id: string }[]) => list.map((e) => e.id);

/** A fixture run as the backend would serve it for `anchor`. */
function payload(key: string, anchor: string, complete = true): FranchiseData {
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
		complete
	};
}

const data = (key: string, anchor: string, rows: HistoryRow[] = []): WatchOrderData => ({
	franchise: payload(key, anchor),
	rows
});

/** Every main-story row, in the page's reading order. */
function rowsOf(page: FranchisePageModel): FranchiseRow[] {
	return page.sections.flatMap((s) => (s.kind === 'ways' ? s.ways.flat() : [...s.rows]));
}

describe('franchisePage', () => {
	it('has no page when the franchise could not be read, answers another title, or is empty', () => {
		expect(franchisePage({ franchise: null, rows: [] }, '7158', 'linked')).toBeNull();
		expect(franchisePage(data('jojo', '7158'), '8063', 'linked')).toBeNull();
		const empty = { ...payload('jojo', '7158'), entries: [], links: [] };
		expect(franchisePage({ franchise: empty, rows: [] }, '7158', 'linked')).toBeNull();
	});

	it('names JoJo after its first title, keeping "(TV)" beside the bare-titled OVA', () => {
		const page = franchisePage(data('jojo', '46598'), '46598', 'linked')!;
		expect(page.art.id).toBe('7158');
		expect(page.name).toBe('JoJo no Kimyou na Bouken (TV)');
	});

	it('drops "(TV)" from the name where no other title would read the same', () => {
		const f = payload('jojo', '46598');
		const renamed = {
			...f,
			entries: f.entries.map((e) => (e.id === '7158' ? { ...e, title: 'Saga (TV)' } : e))
		};
		expect(franchisePage({ franchise: renamed, rows: [] }, '46598', 'linked')!.name).toBe('Saga');
	});

	it('takes its artwork from the first title in link order, whatever the order setting', () => {
		for (const order of ['linked', 'released'] as const) {
			expect(franchisePage(data('fate', '6028'), '6028', order)!.art.id).toBe('6028');
		}
	});

	it('counts every title it lists', () => {
		const page = franchisePage(data('jojo', '46598'), '46598', 'linked')!;
		// 9 in the main story, 4 other versions, 1 side story, 1 recap.
		expect(page.total).toBe(15);
		expect(ids(page.versions)).toEqual(ids(run('jojo').versions));
		expect(ids(page.sides)).toEqual(ids(run('jojo').sides));
		expect(ids(page.recaps)).toEqual(ids(run('jojo').recaps));
	});

	it('resumes the title being watched, with its place and episode', () => {
		const rows: HistoryRow[] = [
			{ kitsu_id: '7158', ep_no: '26' },
			{ kitsu_id: '49847', ep_no: '1' }
		];
		const page = franchisePage(data('jojo', '41410', rows), '41410', 'linked')!;
		expect(page.resume).toMatchObject({ entry: { id: '49847' }, episode: 1, position: 9 });
		expect(page.resume!.label).not.toBe('');
	});

	it('has nothing to resume when nothing is part-way, and starts from the first title', () => {
		const page = franchisePage(data('jojo', '41410'), '41410', 'linked')!;
		expect(page.resume).toBeNull();
		expect(page.start?.id).toBe('7158');
	});

	it('starts from the earliest release in release order', () => {
		// Fate/stay night (2006) came out before Fate/Zero (2011).
		expect(franchisePage(data('fate', '6028'), '6028', 'released')!.start?.id).toBe('323');
	});

	it('numbers a straight run of seasons from 01, marking the title it was opened from', () => {
		const page = franchisePage(data('jojo', '46598'), '46598', 'linked')!;
		expect(page.sections.map((s) => s.kind)).toEqual(['line']);
		const rows = rowsOf(page);
		expect(rows.map((r) => r.position)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
		expect(rows.filter((r) => r.here).map((r) => r.entry.id)).toEqual(['46598']);
	});

	it('shows progress per title, a watched one in full', () => {
		const rows: HistoryRow[] = [
			{ kitsu_id: '7158', ep_no: '26' },
			{ kitsu_id: '8063', ep_no: '6' }
		];
		const page = franchisePage(data('jojo', '46598', rows), '46598', 'linked')!;
		const row = (id: string) => rowsOf(page).find((r) => r.entry.id === id)!;
		expect(row('7158')).toMatchObject({ progress: 'watched', share: 1 });
		expect(row('8063').progress).toBe(6);
		expect(row('8063').share).toBeCloseTo(6 / 24);
		expect(row('8739')).toMatchObject({ progress: undefined, share: 0 });
	});

	it("lays Monogatari's titles with no clear order out as one group, unnumbered", () => {
		const page = franchisePage(data('monogatari', '3919'), '3919', 'linked')!;
		expect(page.sections.map((s) => s.kind)).toEqual(['line', 'group', 'line']);
		const group = page.sections[1];
		expect(group.kind === 'group' && group.rows.length).toBe(10);
		expect(group.kind === 'group' && group.rows.every((r) => r.position === null)).toBe(true);
		const after = page.sections[2];
		expect(after.kind === 'line' && after.unplaced).toBe(true);
		const first = page.sections[0];
		expect(first.kind === 'line' && first.unplaced).toBe(false);
	});

	it('puts the ways to continue side by side, unnumbered, where the story branches', () => {
		const page = franchisePage(data('fate', '6028'), '6028', 'linked')!;
		const ways = page.sections.find((s) => s.kind === 'ways');
		expect(ways).toBeDefined();
		expect(ways!.kind === 'ways' && ways!.ways.length).toBeGreaterThan(1);
		expect(ways!.kind === 'ways' && ways!.ways.flat().every((r) => r.position === null)).toBe(true);
	});

	it('says which title a stray one leads into', () => {
		// Gintama: The Semi-Final leads into The Final from outside the main line.
		const page = franchisePage(data('gintama', '818'), '818', 'linked')!;
		const semi = rowsOf(page).find((r) => r.entry.id === '43829')!;
		expect(semi.leadsInto).toBe(rowsOf(page).find((r) => r.entry.id === '43683')!.label);
	});

	/** A payload of `ids` linked by `links`, anchored on the first. */
	function shaped(ids: string[], links: [string, string][]): WatchOrderData {
		const entries = ids.map((id, i) => ({
			id,
			title: `T${id}`,
			subtype: 'TV',
			start: `20${10 + i}-01-01`,
			episodes: 12,
			status: 'finished',
			poster: null,
			cover: null
		}));
		return {
			franchise: {
				anchor: ids[0],
				entries,
				links,
				versions: [],
				films: [],
				sides: [],
				recaps: [],
				parents: {},
				complete: true
			},
			rows: []
		};
	}

	it('heads the titles after ways that join back up as having no clear place', () => {
		const page = franchisePage(
			shaped(
				['1', '2', '3', '4', '5'],
				[
					['1', '2'],
					['1', '3'],
					['2', '4'],
					['3', '4'],
					['4', '5']
				]
			),
			'1',
			'linked'
		)!;
		expect(page.sections.map((s) => s.kind)).toEqual(['line', 'ways', 'line']);
		const after = page.sections[2];
		expect(after.kind === 'line' && after.rows.map((r) => r.position)).toEqual([null, null]);
		expect(after.kind === 'line' && after.unplaced).toBe(true);
	});

	it('never draws ways to continue with only one way', () => {
		const page = franchisePage(
			shaped(
				['1', '2', '3', '4', '5'],
				[
					['1', '2'],
					['1', '3'],
					['4', '3'],
					['2', '5']
				]
			),
			'1',
			'linked'
		)!;
		for (const s of page.sections) if (s.kind === 'ways') expect(s.ways.length).toBeGreaterThan(1);
		// Every title with no number says why: what it leads into, or
		// the heading over it.
		for (const s of page.sections) {
			if (s.kind !== 'line') continue;
			for (const r of s.rows)
				if (r.position === null) expect(r.leadsInto !== null || s.unplaced).toBe(true);
		}
	});

	it('gathers the titles with no clear place at the end, with the titles that only lead into them', () => {
		// 1→2→J and R→S→J: S has no number, and R only leads into S.
		const page = franchisePage(
			shaped(
				['1', '2', 'R', 'S', 'J'],
				[
					['1', '2'],
					['2', 'J'],
					['R', 'S'],
					['S', 'J']
				]
			),
			'1',
			'linked'
		)!;
		const last = page.sections.at(-1)!;
		expect(last.kind === 'line' && last.unplaced).toBe(true);
		expect(last.kind === 'line' && last.rows.map((r) => r.entry.id)).toEqual(['R', 'S']);
		const first = page.sections[0];
		expect(first.kind === 'line' && first.rows.map((r) => [r.entry.id, r.position])).toEqual([
			['1', 1],
			['2', 2],
			['J', 3]
		]);
	});

	it('keeps numbered titles that follow each other in one list', () => {
		// 11 → 51 → 14, and 59 → 51, 59 → 31: 59 and 31 have no clear
		// place, and 11, 51 and 14 read as one numbered list. The page is
		// opened from 31, the payload's anchor.
		const page = franchisePage(
			shaped(
				['31', '51', '14', '11', '59'],
				[
					['59', '51'],
					['51', '14'],
					['59', '31'],
					['11', '51']
				]
			),
			'31',
			'linked'
		)!;
		const first = page.sections[0];
		expect(first.kind === 'line' && first.rows.map((r) => [r.entry.id, r.position])).toEqual([
			['11', 1],
			['51', 2],
			['14', 3]
		]);
	});

	it('never sets two lists of the same kind side by side', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const page = franchisePage({ franchise: f, rows }, id, order);
				if (!page) return;
				page.sections.forEach((s, i) => {
					const prev = page.sections[i - 1];
					if (s.kind === 'line' && prev?.kind === 'line')
						expect(prev.unplaced).not.toBe(s.unplaced);
				});
			}),
			{ numRuns: 2000 }
		);
	});

	it('is not alone when the one title it lists is another show', () => {
		const f = payload('jojo', '7158');
		const one = {
			...f,
			entries: f.entries.filter((e) => e.id === '8063'),
			links: [],
			versions: [],
			sides: [],
			recaps: [],
			films: []
		};
		const page = franchisePage({ franchise: one, rows: [] }, '7158', 'linked')!;
		expect(page.alone).toBe(false);
		const self = { ...one, entries: f.entries.filter((e) => e.id === '7158') };
		expect(franchisePage({ franchise: self, rows: [] }, '7158', 'linked')!.alone).toBe(true);
	});

	it('keeps a title that says what it leads into among the numbered ones', () => {
		// Gintama: The Semi-Final leads into The Final, with no heading.
		const page = franchisePage(data('gintama', '818'), '818', 'linked')!;
		expect(page.sections.some((x) => x.kind === 'line' && x.unplaced)).toBe(false);
	});

	it('puts one heading over unnumbered stretches that follow each other', () => {
		const page = franchisePage(
			shaped(
				['1', '2', '3', '4', '5'],
				[
					['1', '2'],
					['1', '3'],
					['4', '3'],
					['2', '5']
				]
			),
			'1',
			'linked'
		)!;
		page.sections.forEach((x, i) => {
			const prev = page.sections[i - 1];
			if (x.kind === 'line' && x.unplaced)
				expect(prev?.kind === 'line' && prev.unplaced).toBe(false);
		});
	});

	it('lists every title in one numbered line in release order', () => {
		const page = franchisePage(data('monogatari', '3919'), '3919', 'released')!;
		expect(page.sections).toHaveLength(1);
		expect(rowsOf(page).map((r) => r.position)).toEqual(
			run('monogatari').entries.map((_, i) => i + 1)
		);
	});

	it('says when the franchise read stopped early', () => {
		const partial = { franchise: payload('jojo', '7158', false), rows: [] };
		expect(franchisePage(partial, '7158', 'linked')!.partial).toBe(true);
		expect(franchisePage(data('jojo', '7158'), '7158', 'linked')!.partial).toBe(false);
	});

	it('claims no place, total or beginning for a franchise read in part', () => {
		// The walk goes both ways from the show, so it may have missed
		// earlier titles: nothing counted from the first title it reached
		// is sure.
		for (const order of ['linked', 'released'] as const) {
			const rows = [{ kitsu_id: '7158', ep_no: '3' }];
			const page = franchisePage(
				{ franchise: payload('jojo', '7158', false), rows },
				'7158',
				order
			)!;
			expect(rowsOf(page).every((r) => r.position === null)).toBe(true);
			expect(page.total).toBeNull();
			expect(page.start).toBeNull();
			expect(page.resume?.entry.id).toBe('7158');
			expect(page.resume?.position).toBeNull();
		}
	});

	it('is never alone when read in part: the walk may have missed the rest', () => {
		const self = {
			...payload('jojo', '7158', false),
			entries: run('jojo').entries.filter((e) => e.id === '7158'),
			links: [],
			versions: [],
			films: [],
			sides: [],
			recaps: []
		};
		expect(franchisePage({ franchise: self, rows: [] }, '7158', 'linked')!.alone).toBe(false);
	});

	it('opens for a side story, which sits among the side stories rather than the main story', () => {
		const page = franchisePage(data('fate', '6928'), '6928', 'linked')!;
		expect(ids(page.sides)).toContain('6928');
		expect(rowsOf(page).some((r) => r.here)).toBe(false);
	});

	// Mostly payloads that answer the page's own id, with titles around
	// the run; sometimes any payload, malformed links included.
	const anyPage = fc.oneof(franchiseAroundArb(), franchiseArb()).chain((f) =>
		fc.record({
			f: fc.constant(f),
			id: fc.oneof(
				{ weight: 3, arbitrary: fc.constant(f.anchor) },
				{ weight: 1, arbitrary: fc.constantFrom(f.anchor, ...f.entries.map((e) => e.id)) }
			),
			order: fc.constantFrom('linked' as const, 'released' as const),
			rows: fc.array(
				fc.record({
					kitsu_id: fc.option(fc.constantFrom(...f.entries.map((e) => e.id), '0'), {
						nil: undefined
					}),
					ep_no: fc.constantFrom('1', '2', '12', '30', 'x')
				}),
				{ maxLength: 6 }
			)
		})
	);

	it('has a page exactly where the payload answers this title with a run', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const got = franchisePage({ franchise: f, rows }, id, order);
				expect(got === null).toBe(f.anchor !== id || runOf(f) === null);
			})
		);
	});

	it('lists each title of the run once, numbered as its placement says', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const page = franchisePage({ franchise: f, rows }, id, order);
				if (!page) return;
				const listed = rowsOf(page);
				expect(new Set(ids(listed.map((r) => r.entry)))).toEqual(new Set(ids(page.run.entries)));
				expect(listed).toHaveLength(page.run.entries.length);
				for (const s of page.sections) {
					if (s.kind !== 'line') continue;
					for (const r of s.rows) {
						expect(r.position).toBe(placement(page.run, r.entry.id, order).position);
					}
				}
				for (const r of listed) {
					expect(r.here).toBe(r.entry.id === id);
					const note = pathNote(page.run, r.entry.id, order);
					expect(r.leadsInto !== null).toBe(note?.kind === 'leads-into');
					expect(r.share).toBeGreaterThanOrEqual(0);
					expect(r.share).toBeLessThanOrEqual(1);
				}
			})
		);
	});

	it('follows the ordering: its sections are the segments, in release order one line', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const page = franchisePage({ franchise: f, rows }, id, order);
				if (!page) return;
				// The ways and groups come in the segments' order. A fork
				// left with one way is drawn as a line, and the lines'
				// titles may move to the stretch with no clear place.
				const shapes = (kinds: string[]) => kinds.filter((k) => k !== 'line');
				expect(shapes(page.sections.map((s) => s.kind))).toEqual(
					shapes(
						segments(page.run, order).map((s) =>
							s.kind === 'cycle'
								? 'group'
								: s.kind === 'fork' && s.branches.length > 1
									? 'ways'
									: 'line'
						)
					)
				);
				if (order === 'released') expect(page.sections).toHaveLength(1);
				expect(page.start).toBe(f.complete ? sequence(page.run, order)[0] : null);
			})
		);
	});

	it('explains every title it cannot number, and draws ways only in twos or more', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const page = franchisePage({ franchise: f, rows }, id, order);
				if (!page) return;
				const lines = page.sections.filter((s) => s.kind === 'line');
				const unplaced = lines.filter((s) => s.unplaced);
				// At most one stretch with no clear place, and it comes last,
				// so nothing numbered ever reads as part of it.
				expect(unplaced.length).toBeLessThanOrEqual(1);
				if (unplaced.length === 1) expect(page.sections.at(-1)).toBe(unplaced[0]);
				for (const s of page.sections) {
					if (s.kind === 'ways') expect(s.ways.length).toBeGreaterThan(1);
					if (s.kind !== 'line' || s.unplaced) continue;
					// Outside it, a title with no number leads into a title
					// that has a place.
					for (const r of s.rows) {
						if (r.position !== null) continue;
						const note = pathNote(page.run, r.entry.id, order);
						expect(note?.kind).toBe('leads-into');
						const target = note?.kind === 'leads-into' ? note.entry.id : '';
						expect(unplaced[0]?.rows.some((u) => u.entry.id === target) ?? false).toBe(false);
					}
				}
			})
		);
	});

	it('says the franchise is the show alone only when it lists the show and nothing else', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const page = franchisePage({ franchise: f, rows }, id, order);
				if (!page) return;
				const r = page.run;
				const listed = [...r.entries, ...r.versions, ...r.films, ...r.sides, ...r.recaps];
				expect(page.alone).toBe(f.complete && listed.length === 1 && listed[0].id === id);
			})
		);
	});

	it('names itself after its first title in link order, and counts what it lists', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const page = franchisePage({ franchise: f, rows }, id, order);
				if (!page) return;
				expect(page.art.id).toBe(page.run.nameId);
				expect([page.art.title, withoutFormatSuffix(page.art.title)]).toContain(page.name);
				const r = page.run;
				expect(page.total).toBe(
					f.complete
						? r.entries.length +
								r.versions.length +
								r.films.length +
								r.sides.length +
								r.recaps.length
						: null
				);
				expect(page.partial).toBe(!f.complete);
			})
		);
	});

	it('resumes the title the strip calls the one being watched', () => {
		fc.assert(
			fc.property(anyPage, ({ f, id, order, rows }) => {
				const page = franchisePage({ franchise: f, rows }, id, order);
				if (!page) return;
				const progress = Object.fromEntries(
					rowsOf(page)
						.filter((r) => r.progress !== undefined)
						.map((r) => [r.entry.id, r.progress!])
				);
				const now = currentIn(page.run, progress, order);
				expect(page.resume?.entry.id ?? null).toBe(now?.entry.id ?? null);
				expect(page.resume?.episode ?? null).toBe(now?.episode ?? null);
				if (page.resume) {
					expect(page.resume.position).toBe(
						f.complete ? placement(page.run, page.resume.entry.id, order).position : null
					);
				}
			})
		);
	});
});

describe('franchiseTrail', () => {
	it('reads Home, the show it was opened from, then Franchise', () => {
		const page = franchisePage(data('jojo', '41410'), '41410', 'linked')!;
		expect(franchiseTrail(page, '41410')).toEqual([
			{ label: 'Home', href: '/' },
			{ label: "JoJo's Bizarre Adventure: Golden Wind", href: '/anime/41410' },
			{ label: 'Franchise' }
		]);
	});

	it('names a side story it was opened from, which sits outside the main story', () => {
		const page = franchisePage(data('fate', '6928'), '6928', 'linked')!;
		const side = run('fate').sides.find((e) => e.id === '6928')!;
		expect(franchiseTrail(page, '6928')[1]).toEqual({ label: side.title, href: '/anime/6928' });
	});

	it('always ends on Franchise, with the show between Home and it when the page lists it', () => {
		fc.assert(
			fc.property(
				fc
					.oneof(franchiseAroundArb(), franchiseArb())
					.chain((f) =>
						fc.record({ f: fc.constant(f), id: fc.constantFrom(f.anchor, ...ids(f.entries)) })
					),
				({ f, id }) => {
					const page = franchisePage({ franchise: f, rows: [] }, id, 'linked');
					if (!page) return;
					const trail = franchiseTrail(page, id);
					expect(trail[0]).toEqual({ label: 'Home', href: '/' });
					expect(trail.at(-1)).toEqual({ label: 'Franchise' });
					const listed = [...f.entries, ...f.versions, ...f.films, ...f.sides, ...f.recaps].some(
						(e) => e.id === id
					);
					expect(trail).toHaveLength(listed ? 3 : 2);
					if (listed) expect(trail[1].href).toBe(`/anime/${id}`);
				}
			)
		);
	});
});
