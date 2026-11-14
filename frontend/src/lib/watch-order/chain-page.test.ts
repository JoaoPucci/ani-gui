import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { m } from '$lib/paraglide/messages';
import { watchOrderPage } from './page-model';
import { columnCountText } from './strip-copy';
import type { ChainData, FranchiseData, RunLink, WatchEntry } from './types';

const e = (id: string, start: string, subtype = 'movie'): WatchEntry => ({
	id,
	title: `T${id}`,
	subtype,
	start,
	episodes: 1,
	status: 'finished',
	poster: null,
	cover: null
});

const ids = (list: readonly WatchEntry[]) => list.map((x) => x.id);

// Fate's main story, and the Heaven's Feel films that branch off after
// Fate/Zero 2nd Season (7658): the shape the backend serves them in.
const fz = e('6028', '2011-10-02', 'TV');
const fz2 = e('7658', '2012-04-08', 'TV');
const fsn = e('323', '2006-01-07', 'TV');
const hf1 = e('8675', '2017-10-14');
const hf2 = e('12007', '2019-01-12');
const hf3 = e('12008', '2020-08-15');

function fate(anchor: string, chain: ChainData | undefined, complete = true): FranchiseData {
	return {
		anchor,
		entries: [fsn, fz, fz2],
		links: [
			['6028', '7658'],
			['7658', '323']
		],
		versions: [],
		films: [hf1, hf2, hf3],
		sides: [],
		recaps: [],
		parents: {},
		complete,
		chain
	};
}

const heavensFeel: ChainData = {
	entries: [fz2, hf1, hf2, hf3],
	links: [
		['7658', '8675'],
		['8675', '12007'],
		['12007', '12008']
	],
	seams: ['7658']
};

describe('watchOrderPage on a page outside the main story', () => {
	it("numbers the chain's own titles among themselves: Heaven's Feel II is 02 of 03", () => {
		const p = watchOrderPage(
			{ franchise: fate('12007', heavensFeel), rows: [] },
			'12007',
			'linked'
		)!;
		expect(p.outside).toBe(true);
		expect(p.caption).toEqual({ kind: 'position', position: 2, total: 3 });
		expect(p.eyebrow).toEqual({ position: 2, total: 3 });
		expect(ids(p.view.before)).toEqual(['8675']);
		expect(ids(p.view.after)).toEqual(['12008']);
		expect(p.seams).toEqual({ before: null, after: null });
	});

	it('shows the main-story title it branches off from, uncounted, as where it comes from', () => {
		const p = watchOrderPage({ franchise: fate('8675', heavensFeel), rows: [] }, '8675', 'linked')!;
		expect(p.caption).toEqual({ kind: 'position', position: 1, total: 3 });
		expect(ids(p.view.before)).toEqual(['7658']);
		expect(p.seams).toEqual({ before: 'from', after: null });
	});

	it('ends at its own last title when it leads nowhere', () => {
		const p = watchOrderPage(
			{ franchise: fate('12008', heavensFeel), rows: [] },
			'12008',
			'linked'
		)!;
		expect(p.caption).toEqual({ kind: 'position', position: 3, total: 3 });
		expect(p.view.after).toEqual([]);
	});

	it('goes on into the main story where it leads into it, still counting only its own', () => {
		const f1 = e('6084', '2012-02-04');
		const f2 = e('6664', '2012-06-23');
		const f3 = e('6665', '2013-02-01');
		const tv = e('11655', '2016-07-01', 'TV');
		const franchise: FranchiseData = {
			anchor: '6665',
			entries: [e('24', '1997-10-08', 'TV'), tv],
			links: [['24', '11655']],
			versions: [],
			films: [f1, f2, f3],
			sides: [],
			recaps: [],
			parents: {},
			complete: true,
			chain: {
				entries: [f1, f2, f3, tv],
				links: [
					['6084', '6664'],
					['6664', '6665'],
					['6665', '11655']
				],
				seams: ['11655']
			}
		};
		const p = watchOrderPage({ franchise, rows: [] }, '6665', 'linked')!;
		expect(p.caption).toEqual({ kind: 'position', position: 3, total: 3 });
		expect(ids(p.view.after)).toEqual(['11655']);
		expect(p.seams).toEqual({ before: null, after: 'into' });
	});

	it('keys a seam by the column it stands in, in either order', () => {
		const tv = e('7158', '2012-10-06', 'TV');
		const old = e('610', '1993-11-19', 'OVA');
		const ova = e('7730', '2000-05-25', 'OVA');
		const franchise: FranchiseData = {
			anchor: '7730',
			entries: [tv, e('8063', '2014-04-05', 'TV')],
			links: [['7158', '8063']],
			versions: [old, ova],
			films: [],
			sides: [],
			recaps: [],
			parents: {},
			complete: true,
			chain: {
				entries: [old, ova, tv],
				links: [
					['7158', '7730'],
					['7730', '610']
				],
				seams: ['7158']
			}
		};
		const linked = watchOrderPage({ franchise, rows: [] }, '7730', 'linked')!;
		expect(linked.caption).toEqual({ kind: 'position', position: 1, total: 2 });
		expect(ids(linked.view.before)).toEqual(['7158']);
		expect(linked.seams).toEqual({ before: 'from', after: null });
		const released = watchOrderPage({ franchise, rows: [] }, '7730', 'released')!;
		expect(released.caption).toEqual({ kind: 'position', position: 2, total: 2 });
		expect(ids(released.view.before)).toEqual(['610']);
		expect(ids(released.view.after)).toEqual(['7158']);
		expect(released.seams).toEqual({ before: null, after: 'into' });
	});

	it('reads a chain of one film as a film, with no number', () => {
		const ubw = e('4788', '2010-01-23');
		const chain: ChainData = { entries: [fz2, ubw], links: [['7658', '4788']], seams: ['7658'] };
		const p = watchOrderPage({ franchise: fate('4788', chain), rows: [] }, '4788', 'linked')!;
		expect(p.caption).toEqual({ kind: 'film' });
		expect(p.eyebrow).toBeNull();
		expect(p.seams).toEqual({ before: 'from', after: null });
	});

	it('reads a chain of one title that is not a film with no number', () => {
		const ova = e('5000', '2013-01-01', 'OVA');
		const chain: ChainData = { entries: [fz2, ova], links: [['7658', '5000']], seams: ['7658'] };
		const p = watchOrderPage({ franchise: fate('5000', chain), rows: [] }, '5000', 'linked')!;
		expect(p.caption).toEqual({ kind: 'alone' });
		expect(p.eyebrow).toBeNull();
		expect(p.outside).toBe(true);
	});

	it('draws a side story of a chain title under it, on the chain, outside the main story', () => {
		const special = e('9000', '2018-01-01', 'special');
		const franchise: FranchiseData = {
			...fate('9000', heavensFeel),
			sides: [special],
			parents: { '9000': ['8675'] }
		};
		const p = watchOrderPage({ franchise, rows: [] }, '9000', 'linked')!;
		expect(p.view.kind).toBe('side');
		expect(ids(p.view.before)).toEqual(['8675']);
		expect(p.caption).toEqual({ kind: 'side' });
		expect(p.outside).toBe(true);
		expect(p.seams).toEqual({ before: null, after: null });
	});

	it('keys a column that holds a seam beside an own title as usual, and counts both', () => {
		// Heaven's Feel I forks: on to II, and back into the 2006 series.
		const chain: ChainData = {
			entries: [fsn, fz2, hf1, hf2, hf3],
			links: [
				['7658', '8675'],
				['8675', '12007'],
				['8675', '323'],
				['12007', '12008']
			],
			seams: ['7658', '323']
		};
		const p = watchOrderPage({ franchise: fate('8675', chain), rows: [] }, '8675', 'linked')!;
		expect(ids(p.view.after).sort()).toEqual(['12007', '323']);
		expect(p.seams.after).toBeNull();
		expect(columnCountText(p.view.kind, 'after', p.view.after.length, p.complete)).toBe(
			m.watchorder_strip_ways({ count: 2 })
		);
		expect(p.caption).toEqual({ kind: 'position', position: 1, total: 3 });
	});

	it('in release order, places a seam by date, between two own titles where it falls', () => {
		const a = e('100', '2001-01-01');
		const b = e('101', '2015-01-01');
		const s = e('1', '2010-01-01', 'TV');
		const franchise = (anchor: string): FranchiseData => ({
			...fate(anchor, undefined),
			entries: [s],
			links: [],
			films: [a, b],
			chain: {
				entries: [a, s, b],
				links: [
					['100', '1'],
					['100', '101']
				],
				seams: ['1']
			}
		});
		const first = watchOrderPage({ franchise: franchise('100'), rows: [] }, '100', 'released')!;
		expect(first.caption).toEqual({ kind: 'position', position: 1, total: 2 });
		expect(ids(first.view.after)).toEqual(['1']);
		expect(first.seams.after).toBe('into');
		const second = watchOrderPage({ franchise: franchise('101'), rows: [] }, '101', 'released')!;
		expect(second.caption).toEqual({ kind: 'position', position: 2, total: 2 });
		expect(ids(second.view.before)).toEqual(['1']);
		expect(second.seams.before).toBe('from');
	});

	it('reads no number in a franchise read in part, and still says it is outside', () => {
		const p = watchOrderPage(
			{ franchise: fate('12007', heavensFeel, false), rows: [] },
			'12007',
			'linked'
		)!;
		expect(p.outside).toBe(true);
		expect(p.caption).toEqual({ kind: 'partial' });
		expect(p.eyebrow).toBeNull();
	});

	it('is not outside on a main-story page', () => {
		const p = watchOrderPage({ franchise: fate('7658', undefined), rows: [] }, '7658', 'linked')!;
		expect(p.outside).toBe(false);
		expect(p.seams).toEqual({ before: null, after: null });
	});

	it('counts every own title of a line, and never a seam', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 2, max: 6 }),
				fc.boolean(),
				fc.boolean(),
				fc.constantFrom('linked' as const, 'released' as const),
				(n, from, into, order) => {
					const own = Array.from({ length: n }, (_, i) => e(`${100 + i}`, `${2001 + i}-01-01`));
					const before = e('1', '2000-01-01', 'TV');
					const after = e('2', '2010-01-01', 'TV');
					const seams = [...(from ? [before] : []), ...(into ? [after] : [])];
					const links: RunLink[] = own.slice(1).map((x, i) => [own[i].id, x.id] as const);
					if (from) links.unshift(['1', own[0].id]);
					if (into) links.push([own[n - 1].id, '2']);
					const franchise: FranchiseData = {
						anchor: '',
						entries: [before, after],
						links: [['1', '2']],
						versions: [],
						films: own,
						sides: [],
						recaps: [],
						parents: {},
						complete: true,
						chain: {
							entries: [
								...seams.filter((s) => s.id === '1'),
								...own,
								...seams.filter((s) => s.id === '2')
							],
							links,
							seams: seams.map((s) => s.id)
						}
					};
					own.forEach((x, i) => {
						const p = watchOrderPage(
							{ franchise: { ...franchise, anchor: x.id }, rows: [] },
							x.id,
							order
						)!;
						expect(p.caption).toEqual({ kind: 'position', position: i + 1, total: n });
						expect(p.seams.before).toBe(i === 0 && from ? 'from' : null);
						expect(p.seams.after).toBe(i === n - 1 && into ? 'into' : null);
					});
				}
			)
		);
	});
});
