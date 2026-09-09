import { describe, expect, it } from 'vitest';
import { altTitlesFromKitsu, yearFromKitsuRef, type KitsuAnimeRef } from '$lib/api';
import { handoffArgs } from './handoff-args';

const detail: KitsuAnimeRef = {
	id: '42',
	canonical_title: 'The Show',
	titles: { en: 'The Show', en_jp: 'Za Shou' },
	synopsis: null,
	poster_image: null,
	cover_image: null,
	episode_count: 12,
	subtype: 'TV',
	start_date: '2019-04-07',
	status: 'finished'
} as unknown as KitsuAnimeRef;

describe('handoffArgs', () => {
	it('sends the same request the embedded play sends, the Kitsu id included', () => {
		const args = handoffArgs({
			title: 'The Show',
			episode: 3,
			kitsuId: '42',
			config: { mode: 'dub', quality: '720' },
			detail
		});
		expect(args).toEqual({
			title: 'The Show',
			episode: '3',
			mode: 'dub',
			quality: '720',
			episode_count: 12,
			year: yearFromKitsuRef(detail),
			subtype: 'TV',
			alt_titles: altTitlesFromKitsu(detail),
			kitsu_id: '42'
		});
	});

	it('defaults to sub and best, and sends no id when the page has none', () => {
		const args = handoffArgs({
			title: 'The Show',
			episode: 1,
			kitsuId: '',
			config: { mode: 'weird', quality: '' },
			detail: null
		});
		expect(args.mode).toBe('sub');
		expect(args.quality).toBe('best');
		expect(args.episode_count).toBeNull();
		expect(args.subtype).toBeNull();
		expect(args.alt_titles).toEqual([]);
		expect(args.kitsu_id).toBeUndefined();
	});
});
