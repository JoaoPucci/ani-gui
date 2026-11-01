import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';
import { rankedPick, rowYear } from './pick-rank';
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

describe('rowYear', () => {
	it("reads a trailing year in the row's title first", () => {
		expect(rowYear('The Melancholy of Haruhi Suzumiya: 2nd Season (2009)', null)).toBe(2009);
		expect(rowYear('Show (2009)', 2012)).toBe(2009);
	});

	it("falls back to the year the row's key carries", () => {
		expect(rowYear("JoJo's Bizarre Adventure", 2000)).toBe(2000);
		expect(rowYear('Jujutsu Kaisen (TV)', 2020)).toBe(2020);
	});

	it('names none when neither does', () => {
		expect(rowYear('Black Clover', null)).toBeNull();
		expect(rowYear('Black Clover', undefined)).toBeNull();
		expect(rowYear('Mob Psycho 100', null)).toBeNull();
	});
});

describe('rankedPick', () => {
	const special = hit('12625', 'Black Clover: Jump Festa 2016 Special', '2016-11-27');
	const series = hit('13209', 'Black Clover', '2017-10-03', { en: 'Black Clover' });

	it("takes the hit whose title is the row's", () => {
		expect(rankedPick([special, series], 'Black Clover', null)?.id).toBe('13209');
	});

	it('reads a localized title, and ignores case, apostrophes and punctuation', () => {
		const tv = hit('7158', 'JoJo no Kimyou na Bouken (TV)', '2012-10-06', {
			en_us: 'JoJo’s Bizarre Adventure'
		});
		expect(rankedPick([special, tv], "jojo's bizarre adventure", null)?.id).toBe('7158');
		const judge = hit('8272', 'Sengoku Basara: Judge End', '2014-07-06', {
			en: 'Sengoku BASARA: End of Judgement'
		});
		expect(rankedPick([special, judge], 'Sengoku BASARA - End of Judgement', null)?.id).toBe(
			'8272'
		);
	});

	it("takes the hit from the row's year before one titled as the row", () => {
		const tv = hit('7158', 'JoJo no Kimyou na Bouken (TV)', '2012-10-06', {
			en_us: "JoJo's Bizarre Adventure"
		});
		const ova = hit('7730', 'JoJo no Kimyou na Bouken: Adventure', '2000-05-25', {
			en: "JoJo's Bizarre Adventure (2000)"
		});
		expect(rankedPick([tv, ova], "JoJo's Bizarre Adventure", 2000)?.id).toBe('7730');
	});

	it('names none when no hit carries the year or the title', () => {
		expect(rankedPick([special, series], 'Black Clover Season 2', null)).toBeNull();
		expect(rankedPick([special, series], 'Something Else', 1999)).toBeNull();
		expect(rankedPick([], 'Black Clover', 2017)).toBeNull();
	});
});

// The ranking over arbitrary hits: it answers one of the candidates or
// none, honours the year whenever a candidate carries it, and otherwise
// honours an exact title whenever one carries that.
describe('rankedPick — properties', () => {
	const word = fc.stringMatching(/^[a-z]{2,6}$/);
	const title = fc.array(word, { minLength: 1, maxLength: 3 }).map((w) => w.join(' '));
	const year = fc.integer({ min: 1980, max: 2030 });
	const candidate: fc.Arbitrary<KitsuAnimeRef> = fc
		.record({ id: fc.nat(), title, year, localized: fc.option(title, { nil: undefined }) })
		.map(({ id, title, year, localized }) =>
			hit(`h${id}`, title, `${year}-04-01`, localized ? { en: localized } : {})
		);
	const norm = (s: string) =>
		s
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, ' ')
			.trim();
	const titlesOf = (h: KitsuAnimeRef) => [h.canonical_title, ...Object.values(h.titles ?? {})];

	it('answers one of the candidates, or none', () => {
		fc.assert(
			fc.property(
				fc.array(candidate, { maxLength: 6 }),
				title,
				fc.option(year, { nil: null }),
				(cs, t, y) => {
					const got = rankedPick(cs, t, y);
					expect(got === null || cs.includes(got)).toBe(true);
				}
			)
		);
	});

	it("takes a candidate from the row's year whenever one is", () => {
		fc.assert(
			fc.property(fc.array(candidate, { minLength: 1, maxLength: 6 }), title, (cs, t) => {
				const y = Number((cs[cs.length - 1].start_date ?? '').slice(0, 4));
				expect(rankedPick(cs, t, y)?.start_date?.slice(0, 4)).toBe(String(y));
			})
		);
	});

	it('takes a candidate titled as the row whenever one is and no year applies', () => {
		fc.assert(
			fc.property(fc.array(candidate, { minLength: 1, maxLength: 6 }), fc.nat(), (cs, n) => {
				const named = cs[n % cs.length];
				const t = titlesOf(named)[n % titlesOf(named).length].toUpperCase();
				const got = rankedPick(cs, t, null);
				expect(got && titlesOf(got).some((x) => norm(x ?? '') === norm(t))).toBe(true);
			})
		);
	});

	it('names none when no candidate carries the year or the title', () => {
		fc.assert(
			fc.property(fc.array(candidate, { maxLength: 6 }), title, (cs, t) => {
				const titled = cs.some((h) => titlesOf(h).some((x) => norm(x ?? '') === norm(t)));
				fc.pre(!titled);
				expect(rankedPick(cs, t, 1979)).toBeNull();
			})
		);
	});
});
