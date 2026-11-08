import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { HERO_ROTATION_COUNT, hasRealArt, splitTrending, type HeroArtInput } from './rotation';

interface Show extends HeroArtInput {
	id: string;
}

const BANNER = 'https://s4.anilist.co/file/anilistcdn/media/anime/banner/1.jpg';
const COVER = 'https://media.kitsu.app/anime/1/cover_image/large.jpg';

const banner = (id: string): Show => ({ id, anilist_banner: BANNER, cover_image: null });
const cover = (id: string): Show => ({ id, cover_image: { large: COVER } });
const bare = (id: string): Show => ({ id, anilist_banner: null, cover_image: null });
const ids = (shows: readonly Show[]) => shows.map((s) => s.id);

describe('hasRealArt', () => {
	it('counts an AniList banner or any slot of a Kitsu cover', () => {
		expect(hasRealArt(banner('a'))).toBe(true);
		expect(hasRealArt(cover('a'))).toBe(true);
		expect(hasRealArt({ cover_image: { original: COVER } })).toBe(true);
		expect(hasRealArt({ cover_image: { small: COVER } })).toBe(true);
	});

	it('does not count a show with neither, or a cover with every slot empty', () => {
		expect(hasRealArt(bare('a'))).toBe(false);
		expect(hasRealArt({ cover_image: { large: null, original: null, small: null } })).toBe(false);
		expect(hasRealArt({ anilist_banner: '', cover_image: { large: '' } })).toBe(false);
	});
});

describe('splitTrending', () => {
	it('takes the first three shows with art, in trending order', () => {
		const t = [bare('1'), banner('2'), bare('3'), cover('4'), cover('5'), banner('6')];
		expect(ids(splitTrending(t).hero)).toEqual(['2', '4', '5']);
	});

	it('leads the row with the shows it skipped, then the rest in trending order', () => {
		const t = [bare('1'), banner('2'), bare('3'), cover('4'), cover('5'), banner('6'), bare('7')];
		expect(ids(splitTrending(t).row)).toEqual(['1', '3', '6', '7']);
	});

	it('is the plain top three when every show has art', () => {
		const t = [cover('1'), banner('2'), cover('3'), cover('4')];
		const s = splitTrending(t);
		expect(ids(s.hero)).toEqual(['1', '2', '3']);
		expect(ids(s.row)).toEqual(['4']);
	});

	it('fills the rest of the hero with the first shows without art when fewer than three have it', () => {
		const t = [bare('1'), bare('2'), cover('3'), bare('4'), bare('5')];
		const s = splitTrending(t);
		expect(ids(s.hero)).toEqual(['3', '1', '2']);
		expect(ids(s.row)).toEqual(['4', '5']);
	});

	it('takes every show when there are fewer than three, art first', () => {
		const s = splitTrending([bare('1'), cover('2')]);
		expect(ids(s.hero)).toEqual(['2', '1']);
		expect(s.row).toEqual([]);
	});

	it('is empty for an empty list', () => {
		expect(splitTrending([])).toEqual({ hero: [], row: [] });
	});

	it('rotates through three shows', () => {
		expect(HERO_ROTATION_COUNT).toBe(3);
	});
});

type Art = 'banner' | 'cover' | 'whole' | 'small' | 'both' | 'empty-cover' | 'none';

function make(art: Art, id: string): Show {
	switch (art) {
		case 'banner':
			return banner(id);
		case 'cover':
			return cover(id);
		case 'whole':
			return { id, cover_image: { original: COVER } };
		case 'small':
			return { id, cover_image: { small: COVER } };
		case 'both':
			return { id, anilist_banner: BANNER, cover_image: { large: COVER } };
		case 'empty-cover':
			return { id, anilist_banner: null, cover_image: { large: null, original: null } };
		case 'none':
			return bare(id);
	}
}

const ART: Art[] = ['banner', 'cover', 'whole', 'small', 'both', 'empty-cover', 'none'];

const trendingList = fc
	.array(fc.constantFrom(...ART), { maxLength: 30 })
	.map((arts) => arts.map((art, i) => make(art, String(i))));

const rank = (s: Show) => Number(s.id);
const isAscending = (xs: readonly Show[]) =>
	xs.every((s, i) => i === 0 || rank(xs[i - 1]) < rank(s));

describe('splitTrending properties', () => {
	it('drops nothing and duplicates nothing', () => {
		fc.assert(
			fc.property(trendingList, fc.integer({ min: 0, max: 6 }), (t, n) => {
				const { hero, row } = splitTrending(t, n);
				const all = [...hero, ...row];
				expect(all.length).toBe(t.length);
				expect(new Set(all).size).toBe(t.length);
				for (const s of t) expect(all).toContain(s);
			})
		);
	});

	it('keeps trending order inside the row, and among the hero’s shows with art and without', () => {
		fc.assert(
			fc.property(trendingList, fc.integer({ min: 0, max: 6 }), (t, n) => {
				const { hero, row } = splitTrending(t, n);
				expect(isAscending(row)).toBe(true);
				expect(isAscending(hero.filter(hasRealArt))).toBe(true);
				expect(isAscending(hero.filter((s) => !hasRealArt(s)))).toBe(true);
			})
		);
	});

	it('puts art first: the first shows with art, then the first without it only when those run out', () => {
		fc.assert(
			fc.property(trendingList, fc.integer({ min: 0, max: 6 }), (t, n) => {
				const { hero } = splitTrending(t, n);
				const withArt = t.filter(hasRealArt);
				const without = t.filter((s) => !hasRealArt(s));
				expect(hero.length).toBe(Math.min(n, t.length));
				const artCount = Math.min(n, withArt.length);
				expect(ids(hero.slice(0, artCount))).toEqual(ids(withArt.slice(0, artCount)));
				expect(ids(hero.slice(artCount))).toEqual(ids(without.slice(0, hero.length - artCount)));
			})
		);
	});

	it('lists in the row exactly the shows the hero did not take, in trending order', () => {
		fc.assert(
			fc.property(trendingList, fc.integer({ min: 0, max: 6 }), (t, n) => {
				const { hero, row } = splitTrending(t, n);
				expect(ids(row)).toEqual(ids(t.filter((s) => !hero.includes(s))));
			})
		);
	});

	it('leads the row with the shows the hero skipped', () => {
		fc.assert(
			fc.property(trendingList, (t) => {
				const { hero, row } = splitTrending(t);
				const last = hero.filter(hasRealArt).at(-1);
				fc.pre(last !== undefined);
				const cut = rank(last as Show);
				const skipped = t.filter((s) => rank(s) < cut && !hero.includes(s));
				expect(ids(row.slice(0, skipped.length))).toEqual(ids(skipped));
			})
		);
	});
});
