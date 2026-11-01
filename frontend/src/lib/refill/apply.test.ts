import { describe, expect, it } from 'vitest';
import type { KitsuAnimeRef, KitsuEpisode } from '$lib/api';
import { refillEpisodes, refilledImages } from './apply';

function ep(id: string, number: number, thumb: string | null = null): KitsuEpisode {
	return {
		id,
		canonical_title: null,
		season_number: 1,
		number,
		relative_number: number,
		length: null,
		synopsis: null,
		airdate: null,
		thumbnail: thumb ? { original: thumb } : null
	} as KitsuEpisode;
}

function detail(id: string, cover: string | null): KitsuAnimeRef {
	return {
		id,
		cover_image: cover ? { tiny: null, small: null, large: cover, original: cover } : null,
		anilist_banner: cover
	} as KitsuAnimeRef;
}

describe('refillEpisodes', () => {
	it('fills in only the missing stills, in place, on the cached page and the episodes shown', () => {
		const cached = [ep('a', 1), ep('b', 2, 'https://kitsu/2.jpg')];
		const cache = new Map([[1, cached]]);
		const shown = [ep('a', 1), ep('b', 2, 'https://kitsu/2.jpg')];
		const before = [...shown];
		const titles = shown.map((e) => e.canonical_title);
		refillEpisodes(cache, 1, [ep('a', 1, 'https://x/1.jpg'), ep('b', 2, 'https://x/2.jpg')], shown);
		// The same page, the same array, the same episodes: nothing replaced.
		expect(cache.get(1)).toBe(cached);
		shown.forEach((e, i) => expect(e).toBe(before[i]));
		// Only the missing still is filled; Kitsu's own still stays.
		expect(shown.map((e) => e.thumbnail?.original)).toEqual([
			'https://x/1.jpg',
			'https://kitsu/2.jpg'
		]);
		expect(cached.map((e) => e.thumbnail?.original)).toEqual([
			'https://x/1.jpg',
			'https://kitsu/2.jpg'
		]);
		expect(shown.map((e) => e.canonical_title)).toEqual(titles);
	});

	it('leaves the episodes shown that the refilled page does not carry', () => {
		const cache = new Map([[2, [ep('c', 51)]]]);
		const shown = [ep('b', 50), ep('c', 51)];
		refillEpisodes(cache, 2, [ep('c', 51, 'https://x/51.jpg')], shown);
		expect(shown[0].thumbnail).toBeNull();
		expect(shown[1].thumbnail?.original).toBe('https://x/51.jpg');
	});

	it('leaves another show alone after the page moved on', () => {
		const cache = new Map<number, KitsuEpisode[]>();
		const shown = [ep('z', 1)];
		refillEpisodes(cache, 1, [ep('a', 1, 'https://x/1.jpg')], shown);
		expect(cache.size).toBe(0);
		expect(shown[0].thumbnail).toBeNull();
	});

	it('leaves another show’s page alone when the page moved on to a show with that page cached', () => {
		const other = [ep('z1', 1), ep('z2', 2)];
		const cache = new Map([[1, other]]);
		const shown = [...other];
		refillEpisodes(cache, 1, [ep('a', 1, 'https://x/1.jpg')], shown);
		expect(cache.get(1)).toBe(other);
		expect(other.every((e) => e.thumbnail === null)).toBe(true);
	});

	it('does nothing with no episodes shown or none refilled', () => {
		const cache = new Map([[3, [ep('q', 99)]]]);
		refillEpisodes(cache, 3, [], null);
		const shown = [ep('a', 1)];
		refillEpisodes(cache, 3, [ep('q', 99, 'https://x/q.jpg')], shown);
		expect(shown[0].thumbnail).toBeNull();
	});
});

describe('refilledImages', () => {
	it('takes the cover and banner the refill brought', () => {
		expect(refilledImages(detail('12', null), detail('12', 'https://al/b.jpg'))).toEqual({
			cover_image: {
				tiny: null,
				small: null,
				large: 'https://al/b.jpg',
				original: 'https://al/b.jpg'
			},
			anilist_banner: 'https://al/b.jpg'
		});
	});

	it('changes nothing for another show, no detail, or a refill without a cover', () => {
		expect(refilledImages(detail('13', null), detail('12', 'https://al/b.jpg'))).toBeNull();
		expect(refilledImages(null, detail('12', 'https://al/b.jpg'))).toBeNull();
		expect(refilledImages(detail('12', null), detail('12', null))).toBeNull();
	});
});
