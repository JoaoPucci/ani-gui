import { describe, expect, it, vi } from 'vitest';
import {
	loadWatchLater,
	WATCH_LATER_BRIDGE_MAX_IDS,
	type WatchLaterDeps
} from './watch-later-loader';
import type { ListEntry, Provider } from './types';
import type { KitsuAnimeRef } from '$lib/api';

function row(over: Partial<ListEntry> = {}): ListEntry {
	return {
		provider: 'anilist',
		media_id: 0,
		mal_id: null,
		status: 'planning',
		progress_episodes: 0,
		score_0_to_100: null,
		updated_at_epoch_s: 0,
		title: '',
		...over
	};
}

function kref(id: string): KitsuAnimeRef {
	return {
		id,
		slug: `slug-${id}`,
		canonical_title: `Title ${id}`,
		alt_titles: [],
		poster_image: null,
		cover_image: null,
		synopsis: null,
		episode_count: null,
		episode_length_minutes: null,
		status: null,
		subtype: null,
		start_date: null,
		end_date: null,
		age_rating: null,
		age_rating_guide: null,
		average_rating: null,
		ratings_rank: null,
		popularity_rank: null,
		mal_id: null
	} as unknown as KitsuAnimeRef;
}

describe('loadWatchLater', () => {
	it('returns [] when no provider is in credentials', async () => {
		const deps: WatchLaterDeps = {
			credentials: {},
			fetchCachedList: vi.fn(),
			kitsuByMalIds: vi.fn()
		};
		const out = await loadWatchLater(deps);
		expect(out).toEqual([]);
		expect(deps.fetchCachedList).not.toHaveBeenCalled();
		expect(deps.kitsuByMalIds).not.toHaveBeenCalled();
	});

	it('passes bearer + userId fallback through to fetchCachedList per provider', async () => {
		const fetchCachedList = vi.fn().mockResolvedValue([]);
		const deps: WatchLaterDeps = {
			credentials: {
				anilist: { bearer: 'al-tok', userId: 'al-7' },
				mal: { bearer: 'mal-tok', userId: 'mal-42' }
			},
			fetchCachedList,
			kitsuByMalIds: vi.fn().mockResolvedValue([])
		};
		await loadWatchLater(deps);
		expect(fetchCachedList).toHaveBeenCalledWith('anilist', 'al-tok', 'al-7');
		expect(fetchCachedList).toHaveBeenCalledWith('mal', 'mal-tok', 'mal-42');
	});

	it('merges, dedupes, and bridges to Kitsu in input order', async () => {
		const deps: WatchLaterDeps = {
			credentials: {
				anilist: { bearer: 'a', userId: 'u' },
				mal: { bearer: 'm', userId: 'u' }
			},
			fetchCachedList: vi.fn().mockImplementation((p: Provider) => {
				if (p === 'anilist')
					return Promise.resolve([
						row({ provider: 'anilist', media_id: 1, mal_id: 11, status: 'planning' }),
						row({ provider: 'anilist', media_id: 2, mal_id: 22, status: 'planning' })
					]);
				return Promise.resolve([
					row({ provider: 'mal', media_id: 22, mal_id: 22, status: 'planning' }), // dup
					row({ provider: 'mal', media_id: 33, mal_id: 33, status: 'planning' })
				]);
			}),
			kitsuByMalIds: vi.fn().mockResolvedValue([kref('a'), kref('b'), kref('c')])
		};
		const out = await loadWatchLater(deps);
		expect(deps.kitsuByMalIds).toHaveBeenCalledWith([11, 22, 33]); // dedup, AniList first
		expect(out).toHaveLength(3);
	});

	it('survives a per-provider fetch failure (rail still renders the surviving provider)', async () => {
		const deps: WatchLaterDeps = {
			credentials: {
				anilist: { bearer: 'a', userId: 'u' },
				mal: { bearer: 'm', userId: 'u' }
			},
			fetchCachedList: vi.fn().mockImplementation((p: Provider) => {
				if (p === 'anilist')
					return Promise.resolve([row({ media_id: 1, mal_id: 11, status: 'planning' })]);
				return Promise.reject(new Error('5xx'));
			}),
			kitsuByMalIds: vi.fn().mockResolvedValue([kref('a')])
		};
		const out = await loadWatchLater(deps);
		expect(deps.kitsuByMalIds).toHaveBeenCalledWith([11]);
		expect(out).toHaveLength(1);
	});

	// Codex P2 #3373907898: backend rejects >500 mal_ids before
	// fan-out. Without a client-side slice, a heavy listmaker with
	// 501+ planned titles would 4xx the bridge call and lose the
	// entire rail (loader.catch sets watchLater=[]). Slicing here
	// renders the first 500 instead of nothing.
	it('caps the bridge call at WATCH_LATER_BRIDGE_MAX_IDS', async () => {
		const rows: ListEntry[] = Array.from({ length: WATCH_LATER_BRIDGE_MAX_IDS + 50 }, (_, i) =>
			row({ provider: 'anilist', media_id: i + 1, mal_id: i + 1, status: 'planning' })
		);
		const bridge = vi.fn().mockResolvedValue([]);
		const deps: WatchLaterDeps = {
			credentials: { anilist: { bearer: 'a', userId: 'u' } },
			fetchCachedList: vi.fn().mockResolvedValue(rows),
			kitsuByMalIds: bridge
		};
		await loadWatchLater(deps);
		expect(bridge).toHaveBeenCalledTimes(1);
		const arg = (bridge.mock.calls[0] ?? [])[0] as number[];
		expect(arg).toHaveLength(WATCH_LATER_BRIDGE_MAX_IDS);
		// AniList order preserved; first MAX_IDS rows are 1..500.
		expect(arg[0]).toBe(1);
		expect(arg[arg.length - 1]).toBe(WATCH_LATER_BRIDGE_MAX_IDS);
	});

	it('skips the Kitsu round-trip when the merged set has no mal_ids', async () => {
		const deps: WatchLaterDeps = {
			credentials: { anilist: { bearer: 'a', userId: 'u' } },
			fetchCachedList: vi
				.fn()
				.mockResolvedValue([row({ media_id: 1, mal_id: null, status: 'planning' })]),
			kitsuByMalIds: vi.fn().mockResolvedValue([])
		};
		const out = await loadWatchLater(deps);
		expect(deps.kitsuByMalIds).not.toHaveBeenCalled();
		expect(out).toEqual([]);
	});
});
