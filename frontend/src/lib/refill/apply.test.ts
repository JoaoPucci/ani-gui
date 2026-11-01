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
	it('puts the refilled page in the cache and its stills on the episodes shown', () => {
		const cache = new Map([[1, [ep('a', 1), ep('b', 2)]]]);
		const shown = [ep('a', 1), ep('b', 2)];
		const filled = [ep('a', 1, 'https://x/1.jpg'), ep('b', 2, 'https://x/2.jpg')];
		const got = refillEpisodes(cache, 1, filled, shown);
		expect(cache.get(1)).toBe(filled);
		expect(got?.map((e) => e.thumbnail?.original)).toEqual(['https://x/1.jpg', 'https://x/2.jpg']);
	});

	it('keeps the episodes shown that the refilled page does not carry', () => {
		const cache = new Map([[2, [ep('c', 51)]]]);
		const shown = [ep('b', 50), ep('c', 51)];
		const got = refillEpisodes(cache, 2, [ep('c', 51, 'https://x/51.jpg')], shown);
		expect(got?.[0]).toBe(shown[0]);
		expect(got?.[1].thumbnail?.original).toBe('https://x/51.jpg');
	});

	it('leaves another show alone after the page moved on', () => {
		const cache = new Map<number, KitsuEpisode[]>();
		const shown = [ep('z', 1)];
		expect(refillEpisodes(cache, 1, [ep('a', 1, 'https://x/1.jpg')], shown)).toBe(shown);
		expect(cache.size).toBe(0);
	});

	it('leaves another show’s page alone when the page moved on to a show with that page cached', () => {
		const other = [ep('z1', 1), ep('z2', 2)];
		const cache = new Map([[1, other]]);
		const shown = [...other];
		const got = refillEpisodes(cache, 1, [ep('a', 1, 'https://x/1.jpg')], shown);
		expect(cache.get(1)).toBe(other);
		expect(got).toBe(shown);
	});

	it('returns the same episodes when none were refilled', () => {
		const cache = new Map([[3, [ep('q', 99)]]]);
		const shown = [ep('a', 1)];
		expect(refillEpisodes(cache, 3, [ep('q', 99, 'https://x/q.jpg')], shown)).toBe(shown);
		expect(refillEpisodes(cache, 3, [], null)).toBeNull();
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
