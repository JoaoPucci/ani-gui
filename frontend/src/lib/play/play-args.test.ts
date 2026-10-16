import { describe, expect, it } from 'vitest';
import { altTitlesFromKitsu, yearFromKitsuRef, type KitsuAnimeRef } from '$lib/api';
import { buildPlayArgs } from './play-args';

const show: KitsuAnimeRef = {
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

describe('buildPlayArgs', () => {
	it("describes the episode by the show's Kitsu record and names the Kitsu id", () => {
		expect(
			buildPlayArgs({
				title: 'The Show',
				episode: 3,
				mode: 'dub',
				quality: '720',
				show,
				kitsuId: '42'
			})
		).toEqual({
			title: 'The Show',
			episode: '3',
			mode: 'dub',
			quality: '720',
			episode_count: 12,
			year: yearFromKitsuRef(show),
			subtype: 'TV',
			alt_titles: altTitlesFromKitsu(show),
			kitsu_id: '42'
		});
	});

	it('marks a warm as a prefetch, and only a warm', () => {
		const base = { title: 'The Show', episode: 1, mode: 'sub', quality: 'best', show } as const;
		expect(buildPlayArgs({ ...base, prefetch: true }).prefetch).toBe(true);
		expect('prefetch' in buildPlayArgs(base)).toBe(false);
		expect('prefetch' in buildPlayArgs({ ...base, prefetch: false })).toBe(false);
	});

	it('sends no Kitsu id when there is none to record, and nulls without a record', () => {
		const args = buildPlayArgs({
			title: 'The Show',
			episode: 1,
			mode: 'sub',
			quality: 'best',
			show: null,
			kitsuId: ''
		});
		expect('kitsu_id' in args).toBe(false);
		expect(args.episode_count).toBeNull();
		expect(args.year).toBeNull();
		expect(args.subtype).toBeNull();
		expect(args.alt_titles).toEqual([]);
	});
});
