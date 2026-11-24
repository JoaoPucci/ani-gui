import { describe, expect, it, vi } from 'vitest';

import { applyHeroPick, heroShow, NO_SPLIT, pickTrending } from './rotation';
import type { HeroPick, HeroPickBody, KitsuAnimeRef } from '$lib/api';

// The pick itself, and its rules, are the backend's (backend/src/hero,
// whose tests the rules' cases moved to). What is left here is the
// renderer's side of it: what it sends, and how the answer's ids
// become the shows it draws.

const BANNER = 'https://s4.anilist.co/file/anilistcdn/media/anime/banner/1.jpg';
const COVER = 'https://media.kitsu.app/anime/1/cover_image/large.jpg';

function ref(id: string, extra: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id,
		canonical_title: `Show ${id}`,
		titles: {},
		abbreviated_titles: [],
		slug: null,
		synopsis: null,
		start_date: null,
		end_date: null,
		episode_count: null,
		average_rating: null,
		subtype: null,
		status: 'finished',
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null,
		...extra
	};
}

const walk = { airing_first: false, count: 3, entries: 0, steps: [] };

describe('heroShow', () => {
	it('sends the id, the title, the status and the art the pick reads, nothing else', () => {
		const show = ref('7', {
			status: 'current',
			anilist_banner: BANNER,
			cover_image: { tiny: COVER, small: null, large: COVER, original: null },
			synopsis: 'not read'
		});
		expect(heroShow(show)).toEqual({
			id: '7',
			title: 'Show 7',
			status: 'current',
			anilist_banner: BANNER,
			cover_image: { large: COVER, original: null, small: null }
		});
	});

	it('sends no banner and no cover for a show without them', () => {
		expect(heroShow(ref('7', { status: null }))).toEqual({
			id: '7',
			title: 'Show 7',
			status: null,
			anilist_banner: null,
			cover_image: null
		});
	});
});

describe('applyHeroPick', () => {
	it('draws the hero and the row in the order the pick gives', () => {
		const t = [ref('1'), ref('2'), ref('3'), ref('4')];
		const split = applyHeroPick(t, { hero: ['3', '1'], row: ['2', '4'] });
		expect(split.hero).toEqual([t[2], t[0]]);
		expect(split.row).toEqual([t[1], t[3]]);
	});

	it("draws a show's first entry when the list repeats it", () => {
		const first = ref('1', { canonical_title: 'First entry' });
		const t = [first, ref('2'), ref('1', { canonical_title: 'Repeat' })];
		expect(applyHeroPick(t, { hero: ['1'], row: ['2'] }).hero).toEqual([first]);
	});

	it('leaves out an id the list does not hold', () => {
		const t = [ref('1'), ref('2')];
		expect(applyHeroPick(t, { hero: ['9', '1'], row: ['2', '8'] })).toEqual({
			hero: [t[0]],
			row: [t[1]]
		});
	});

	it('is empty for an empty pick', () => {
		expect(applyHeroPick([ref('1')], { hero: [], row: [] })).toEqual(NO_SPLIT);
	});
});

describe('pickTrending', () => {
	it('asks the backend for the pick over every entry, in trending order, and draws its answer', async () => {
		const t = [ref('1'), ref('2', { anilist_banner: BANNER }), ref('1')];
		const ask = vi.fn(async (body: HeroPickBody, opts?: unknown): Promise<HeroPick> => {
			void body;
			void opts;
			return { hero: ['2', '1'], row: [], walk };
		});
		const signal = new AbortController().signal;
		const split = await pickTrending(t, { signal }, ask);
		expect(ask).toHaveBeenCalledOnce();
		expect(ask.mock.calls[0][0]).toEqual({ shows: t.map(heroShow) });
		expect(ask.mock.calls[0][1]).toEqual({ signal });
		expect(split).toEqual({ hero: [t[1], t[0]], row: [] });
	});

	it('fails when the backend does', async () => {
		const ask = vi.fn(async (): Promise<HeroPick> => {
			throw new Error('backend down');
		});
		await expect(pickTrending([ref('1')], {}, ask)).rejects.toThrow('backend down');
	});
});
