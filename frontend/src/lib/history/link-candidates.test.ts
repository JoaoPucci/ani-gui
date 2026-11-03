// The shows a guessed history row can be linked to: the guess's own
// title search and its slug's words, and the warnings a link to one
// of them earns.

import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { linkWarnings, mergeCandidates, slugSearchTerm } from './link-candidates';

function ref(id: string, over: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Show ${id}`,
		slug: `show-${id}`,
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

function row(id: string, title: string, ep_no = '1'): HistoryEntry {
	return { id, title, ep_no };
}

describe('slugSearchTerm', () => {
	it("reads a walked provider's slug without its label and number", () => {
		expect(slugSearchTerm('hianime:jojos-bizarre-adventure-2000-5182')).toBe(
			'jojos bizarre adventure 2000'
		);
	});

	it("reads a retired provider's bare slug", () => {
		expect(slugSearchTerm('naruto-shippuden-3687')).toBe('naruto shippuden');
	});

	it('has none for an id that is not slug-shaped', () => {
		expect(slugSearchTerm('vDTSJHSpYnrkZnAvG')).toBeNull();
		expect(slugSearchTerm('hianime:5182')).toBeNull();
		expect(slugSearchTerm('hianime:Black-Clover-949')).toBeNull();
	});
});

describe('mergeCandidates', () => {
	it('leads with the show shown now, then interleaves the two searches', () => {
		const out = mergeCandidates(
			ref('7158'),
			[ref('7158'), ref('41410'), ref('8739')],
			[ref('7730'), ref('41410')]
		);
		expect(out.map((c) => [c.ref.id, c.current])).toEqual([
			['7158', true],
			['7730', false],
			['41410', false],
			['8739', false]
		]);
	});

	it('offers no music video', () => {
		const out = mergeCandidates(null, [ref('1', { subtype: 'music' }), ref('2')], []);
		expect(out.map((c) => c.ref.id)).toEqual(['2']);
	});

	it('offers each show once, every one searched, the current first', () => {
		const ids = fc.array(fc.constantFrom('1', '2', '3', '4', '5'), { maxLength: 8 });
		fc.assert(
			fc.property(fc.option(fc.constantFrom('1', '2', '9')), ids, ids, (cur, a, b) => {
				const current = cur === null ? null : ref(cur);
				const out = mergeCandidates(
					current,
					a.map((id) => ref(id)),
					b.map((id) => ref(id))
				);
				const got = out.map((c) => c.ref.id);
				expect(new Set(got).size).toBe(got.length);
				expect(new Set(got)).toEqual(new Set([...(cur ? [cur] : []), ...a, ...b]));
				if (cur) expect(out[0]).toEqual({ ref: current, current: true });
				expect(out.filter((c) => c.current).length).toBe(cur ? 1 : 0);
			})
		);
	});
});

describe('linkWarnings', () => {
	it('warns when the saved title names a season the show does not', () => {
		const gintama4 = row('hianime:gintama-season-4-1381', 'Gintama Season 4');
		expect(
			linkWarnings(
				gintama4,
				ref('10083', { canonical_title: 'Gintama°', slug: 'gintama-2015-tv' }),
				1
			)
		).toEqual(['season']);
	});

	it('does not warn when the show carries the same season, by title or slug', () => {
		const so3 = row(
			'hianime:jojos-bizarre-adventure-stone-ocean-part-3-2834',
			"JoJo's Bizarre Adventure: Stone Ocean Part 3"
		);
		expect(
			linkWarnings(
				so3,
				ref('46598', {
					canonical_title: 'JoJo no Kimyou na Bouken: Stone Ocean Part 3',
					slug: 'jojo-no-kimyou-na-bouken-stone-ocean-part-3'
				}),
				1
			)
		).toEqual([]);
		expect(
			linkWarnings(
				row('x-1', 'Show Season 2'),
				ref('5', { canonical_title: 'Show II', slug: 'show-season-2' }),
				1
			)
		).toEqual([]);
	});

	it('warns when the episode the card offers is past the show’s episode count', () => {
		const naruto = row('hianime:naruto-shippuden-600', 'Naruto: Shippuden', '425');
		expect(
			linkWarnings(naruto, ref('12625', { canonical_title: 'Naruto', episode_count: 1 }), 426)
		).toEqual(['episode']);
		expect(
			linkWarnings(naruto, ref('1555', { canonical_title: 'Naruto', episode_count: 500 }), 426)
		).toEqual([]);
		expect(
			linkWarnings(naruto, ref('1555', { canonical_title: 'Naruto', episode_count: null }), 426)
		).toEqual([]);
	});

	it('measures the episode the card offers, not the one the row saved', () => {
		// The row saved episode 1 and the card offers 2: the one-episode
		// special is too short for the card, though not for the row.
		const special = ref('12625', { canonical_title: 'Black Clover Special', episode_count: 1 });
		expect(linkWarnings(row('hianime:black-clover-949', 'Black Clover', '1'), special, 2)).toEqual([
			'episode'
		]);
	});
});
