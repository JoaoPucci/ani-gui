import { describe, expect, it } from 'vitest';
import { mergedWatchLater } from './watch-later';
import type { ListEntry, Provider } from './types';

function entry(over: Partial<ListEntry> = {}): ListEntry {
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

describe.skip('mergedWatchLater', () => {
	it('returns an empty list when no provider has rows', () => {
		expect(mergedWatchLater({})).toEqual([]);
	});

	it('returns only Planning rows; other statuses are dropped', () => {
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			anilist: [
				entry({ media_id: 1, mal_id: 11, status: 'planning', title: 'A' }),
				entry({ media_id: 2, mal_id: 12, status: 'watching', title: 'B' }),
				entry({ media_id: 3, mal_id: 13, status: 'completed', title: 'C' })
			]
		};
		const out = mergedWatchLater(rows);
		expect(out.map((e) => e.media_id)).toEqual([1]);
	});

	it('orders AniList rows before MAL rows', () => {
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			mal: [entry({ provider: 'mal', media_id: 200, mal_id: 200, status: 'planning' })],
			anilist: [entry({ provider: 'anilist', media_id: 10, mal_id: 100, status: 'planning' })]
		};
		const out = mergedWatchLater(rows);
		expect(out.map((e) => e.provider)).toEqual(['anilist', 'mal']);
	});

	it('dedupes across providers on mal_id (AniList wins)', () => {
		const shared = 42;
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			anilist: [
				entry({
					provider: 'anilist',
					media_id: 9001,
					mal_id: shared,
					status: 'planning',
					title: 'AniList copy'
				})
			],
			mal: [
				entry({
					provider: 'mal',
					media_id: shared,
					mal_id: shared,
					status: 'planning',
					title: 'MAL copy'
				})
			]
		};
		const out = mergedWatchLater(rows);
		expect(out).toHaveLength(1);
		expect(out[0].provider).toBe('anilist');
		expect(out[0].title).toBe('AniList copy');
	});

	it('keeps entries with null mal_id (un-dedupable but still rendered)', () => {
		// Rare AniList-only titles with no MAL mapping. Plan §6.6
		// pseudo-code keeps them; the merge must too.
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			anilist: [
				entry({ media_id: 1, mal_id: null, status: 'planning', title: 'Only-AL 1' }),
				entry({ media_id: 2, mal_id: null, status: 'planning', title: 'Only-AL 2' })
			]
		};
		const out = mergedWatchLater(rows);
		expect(out).toHaveLength(2);
	});

	it('does not crash when a provider key is absent', () => {
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			anilist: [entry({ media_id: 1, mal_id: 11, status: 'planning' })]
		};
		expect(mergedWatchLater(rows)).toHaveLength(1);
	});

	it('preserves source order within a provider', () => {
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			anilist: [
				entry({ media_id: 3, mal_id: 33, status: 'planning' }),
				entry({ media_id: 1, mal_id: 11, status: 'planning' }),
				entry({ media_id: 2, mal_id: 22, status: 'planning' })
			]
		};
		expect(mergedWatchLater(rows).map((e) => e.media_id)).toEqual([3, 1, 2]);
	});
});
