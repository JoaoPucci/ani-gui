import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';
import { namedCandidates, rowYear } from './pick-rank';
import type { KitsuAnimeRef } from '$lib/api';

const hit = (
	id: string,
	canonical_title: string,
	start_date: string | null,
	titles: Record<string, string> = {}
): KitsuAnimeRef =>
	({
		id,
		canonical_title,
		titles,
		slug: null,
		start_date,
		episode_count: null,
		subtype: 'TV',
		status: 'finished'
	}) as unknown as KitsuAnimeRef;

const ids = (hits: KitsuAnimeRef[]) => hits.map((h) => h.id);

describe('rowYear', () => {
	it("reads a trailing year in the row's title first", () => {
		expect(rowYear('The Melancholy of Haruhi Suzumiya: 2nd Season (2009)', null)).toBe(2009);
		expect(rowYear('Show (2009)', 2012)).toBe(2009);
	});

	it("falls back to the year the row's key carries", () => {
		expect(rowYear("JoJo's Bizarre Adventure", 2000)).toBe(2000);
		expect(rowYear('Jujutsu Kaisen (TV)', 2020)).toBe(2020);
	});

	it('reads only the years the key reads: 1960 to 2039', () => {
		expect(rowYear('Show (1080)', null)).toBeNull();
		expect(rowYear('Blade Runner (2049)', 2001)).toBe(2001);
		expect(rowYear('Show (1960)', null)).toBe(1960);
	});

	it('names none when neither does', () => {
		expect(rowYear('Black Clover', null)).toBeNull();
		expect(rowYear('Black Clover', undefined)).toBeNull();
		expect(rowYear('Mob Psycho 100', null)).toBeNull();
	});
});

describe('namedCandidates', () => {
	const special = hit('12625', 'Black Clover: Jump Festa 2016 Special', '2016-11-27');
	const series = hit('13209', 'Black Clover', '2017-10-03', { en: 'Black Clover' });

	it("narrows to the hit whose title is the row's", () => {
		expect(ids(namedCandidates([special, series], 'Black Clover', null))).toEqual(['13209']);
	});

	it('reads a localized title, case and the apostrophe glyph aside', () => {
		const tv = hit('7158', 'JoJo no Kimyou na Bouken (TV)', '2012-10-06', {
			en_us: 'JoJo’s Bizarre Adventure'
		});
		expect(ids(namedCandidates([special, tv], "jojo's bizarre adventure", null))).toEqual(['7158']);
	});

	it('takes an exact title before one that differs from it only in punctuation', () => {
		// Kitsu tells Gintama's seasons apart by a mark alone.
		const gintama = [
			hit('5971', "Gintama'", '2011-04-04'),
			hit('818', 'Gintama', '2006-04-04'),
			hit('12553', 'Gintama.', '2017-01-09'),
			hit('10083', 'Gintama°', '2015-04-08')
		];
		expect(ids(namedCandidates(gintama, 'Gintama', null))).toEqual(['818']);
		expect(ids(namedCandidates(gintama, "Gintama'", null))).toEqual(['5971']);
		const working = [hit('w2', "Working'!!", '2011-10-01'), hit('w1', 'Working!!', '2010-04-04')];
		expect(ids(namedCandidates(working, 'Working!!', null))).toEqual(['w1']);
	});

	it('narrows to every hit punctuation alone separates, when none is exact', () => {
		const judge = hit('8272', 'Sengoku Basara: Judge End', '2014-07-06', {
			en: 'Sengoku BASARA: End of Judgement'
		});
		const other = hit('x', 'Sengoku BASARA - End of Judgement!', '2014-07-06');
		expect(
			ids(namedCandidates([special, judge, other], 'Sengoku BASARA - End of Judgement', null))
		).toEqual(['8272', 'x']);
	});

	it("narrows to the hits from the row's year, and leaves the rest to the rules after", () => {
		const s1 = hit('s1', 'Show', '2019-01-10');
		const s2 = hit('s2', 'Show Season 2', '2019-10-02');
		const s0 = hit('s0', 'Show Special', '2018-06-01');
		expect(ids(namedCandidates([s1, s0, s2], 'Show Season 2 (2019)', null))).toEqual(['s1', 's2']);
		const tv = hit('7158', 'JoJo no Kimyou na Bouken (TV)', '2012-10-06', {
			en_us: "JoJo's Bizarre Adventure"
		});
		const ova = hit('7730', 'JoJo no Kimyou na Bouken: Adventure', '2000-05-25');
		expect(ids(namedCandidates([tv, ova], "JoJo's Bizarre Adventure", 2000))).toEqual(['7730']);
	});

	it('leaves the candidates as they are when the row names neither', () => {
		expect(ids(namedCandidates([special, series], 'Black Clover Season 2', null))).toEqual([
			'12625',
			'13209'
		]);
		expect(ids(namedCandidates([special, series], 'Something Else', 1999))).toEqual([
			'12625',
			'13209'
		]);
		expect(namedCandidates([], 'Black Clover', 2017)).toEqual([]);
	});
});

// The narrowing over arbitrary hits: never empty when there were hits,
// always the candidates' own order, every result from the row's year
// whenever one is, and an exact title beating a mark-only difference.
describe('namedCandidates — properties', () => {
	const word = fc.stringMatching(/^[a-z]{2,6}$/);
	const title = fc.array(word, { minLength: 1, maxLength: 3 }).map((w) => w.join(' '));
	const mark = fc.constantFrom('', "'", '.', '°', '!!', ':');
	const year = fc.integer({ min: 1980, max: 2030 });
	const candidate: fc.Arbitrary<KitsuAnimeRef> = fc
		.record({ id: fc.nat(), title, mark, year, localized: fc.option(title, { nil: undefined }) })
		.map(({ id, title, mark, year, localized }) =>
			hit(`h${id}`, `${title}${mark}`, `${year}-04-01`, localized ? { en: localized } : {})
		);
	const fold = (s: string) =>
		s
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, ' ')
			.trim();
	const titlesOf = (h: KitsuAnimeRef) =>
		[h.canonical_title, ...Object.values(h.titles ?? {})].map((t) => t ?? '');

	it('keeps at least one candidate, in the candidates own order', () => {
		fc.assert(
			fc.property(
				fc.array(candidate, { maxLength: 6 }),
				title,
				fc.option(year, { nil: null }),
				(cs, t, y) => {
					const got = namedCandidates(cs, t, y);
					expect(got.length > 0).toBe(cs.length > 0);
					let at = 0;
					for (const g of got) {
						at = cs.indexOf(g, at);
						expect(at).toBeGreaterThanOrEqual(0);
						at += 1;
					}
				}
			)
		);
	});

	it("keeps only candidates from the row's year whenever one is", () => {
		fc.assert(
			fc.property(fc.array(candidate, { minLength: 1, maxLength: 6 }), title, (cs, t) => {
				const y = Number((cs[cs.length - 1].start_date ?? '').slice(0, 4));
				for (const g of namedCandidates(cs, t, y)) {
					expect(g.start_date?.slice(0, 4)).toBe(String(y));
				}
			})
		);
	});

	it('keeps only exactly titled candidates whenever one is, past every mark-only sibling', () => {
		fc.assert(
			fc.property(fc.array(candidate, { minLength: 1, maxLength: 6 }), fc.nat(), (cs, n) => {
				const named = cs[n % cs.length];
				const t = named.canonical_title.toUpperCase();
				for (const g of namedCandidates(cs, t, null)) {
					expect(titlesOf(g).some((x) => x.toLowerCase() === t.toLowerCase())).toBe(true);
				}
			})
		);
	});

	it('keeps only candidates the title names, marks aside, when none is exact', () => {
		fc.assert(
			fc.property(fc.array(candidate, { minLength: 1, maxLength: 6 }), title, (cs, t) => {
				const exact = cs.some((h) => titlesOf(h).some((x) => x.toLowerCase() === t));
				const folded = cs.some((h) => titlesOf(h).some((x) => fold(x) === fold(t)));
				fc.pre(!exact && folded);
				for (const g of namedCandidates(cs, t, null)) {
					expect(titlesOf(g).some((x) => fold(x) === fold(t))).toBe(true);
				}
			})
		);
	});
});
