import { describe, expect, it } from 'vitest';
import { asRef } from './poster-ref';
import type { WatchEntry } from './types';

const ova: WatchEntry = {
	id: '610',
	title: "JoJo's Bizarre Adventure (1993)",
	subtype: 'OVA',
	start: '1993-11-19',
	episodes: 13,
	status: 'finished',
	poster: 'https://media.kitsu.app/anime/poster_images/610/medium.jpg',
	cover: 'https://media.kitsu.app/anime/cover_images/610/large.jpg'
};

describe('asRef', () => {
	it('shapes an entry for PosterCard', () => {
		const ref = asRef(ova);
		expect(ref).toMatchObject({
			id: '610',
			canonical_title: ova.title,
			episode_count: ova.episodes,
			start_date: ova.start,
			subtype: ova.subtype
		});
		expect(ref.poster_image?.medium).toBe(ova.poster);
		expect(ref.cover_image?.large).toBe(ova.cover);
	});

	it('has no images where the entry has none', () => {
		expect(asRef({ ...ova, poster: null, cover: null })).toMatchObject({
			poster_image: null,
			cover_image: null
		});
	});
});
