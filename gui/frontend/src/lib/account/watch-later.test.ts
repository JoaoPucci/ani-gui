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

describe('mergedWatchLater', () => {
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

	it('orders AniList rows before MAL rows when nothing separates them by recency', () => {
		// Both untimestamped, so the walk order is what is left to
		// decide it. Recency leads; provider order is the tie-break.
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			mal: [entry({ provider: 'mal', media_id: 200, mal_id: 200, status: 'planning' })],
			anilist: [entry({ provider: 'anilist', media_id: 10, mal_id: 100, status: 'planning' })]
		};
		const out = mergedWatchLater(rows);
		expect(out.map((e) => e.provider)).toEqual(['anilist', 'mal']);
	});

	it('leads with the primary provider when one is set and recency ties', () => {
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			mal: [entry({ provider: 'mal', media_id: 200, mal_id: 200, status: 'planning' })],
			anilist: [entry({ provider: 'anilist', media_id: 10, mal_id: 100, status: 'planning' })]
		};
		const out = mergedWatchLater(rows, 'mal');
		expect(out.map((e) => e.provider)).toEqual(['mal', 'anilist']);
	});

	it('lets the primary provider win the cross-provider mal_id dedupe', () => {
		const shared = 42;
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			anilist: [
				entry({ provider: 'anilist', media_id: 9001, mal_id: shared, title: 'AniList copy' })
			],
			mal: [entry({ provider: 'mal', media_id: shared, mal_id: shared, title: 'MAL copy' })]
		};
		const out = mergedWatchLater(rows, 'mal');
		expect(out).toHaveLength(1);
		expect(out[0].provider).toBe('mal');
	});

	it('falls back to AniList-first when primary is null/unset', () => {
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			mal: [entry({ provider: 'mal', media_id: 200, mal_id: 200, status: 'planning' })],
			anilist: [entry({ provider: 'anilist', media_id: 10, mal_id: 100, status: 'planning' })]
		};
		expect(mergedWatchLater(rows, null).map((e) => e.provider)).toEqual(['anilist', 'mal']);
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

	it('orders the rail by recency, newest first', () => {
		// The rail used to render in whatever order the providers
		// happened to hand back, so a title planned months ago could
		// sit ahead of one planned yesterday.
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			anilist: [
				entry({ media_id: 1, mal_id: 11, updated_at_epoch_s: 1_700_000_000, title: 'oldest' }),
				entry({ media_id: 2, mal_id: 12, updated_at_epoch_s: 1_700_000_200, title: 'newest' })
			],
			mal: [
				entry({
					provider: 'mal',
					media_id: 3,
					mal_id: 13,
					updated_at_epoch_s: 1_700_000_100,
					title: 'middle'
				})
			]
		};
		expect(mergedWatchLater(rows).map((e) => e.title)).toEqual(['newest', 'middle', 'oldest']);
	});

	it('sends rows the provider never timestamped to the end, not the front', () => {
		// MAL's parser stores 0 when a row carries no `updated_at`.
		// Read as a date that is the beginning of 1970, which is the
		// right end of the rail for something we know nothing about —
		// but only if the comparison is descending, and an ascending
		// one would put it first.
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			anilist: [
				entry({ media_id: 1, mal_id: 11, updated_at_epoch_s: 0, title: 'untimestamped' }),
				entry({ media_id: 2, mal_id: 12, updated_at_epoch_s: 1_700_000_000, title: 'dated' })
			]
		};
		expect(mergedWatchLater(rows).map((e) => e.title)).toEqual(['dated', 'untimestamped']);
	});

	it('resolves a duplicate by provider, not by which copy is newer', () => {
		// The sort must run AFTER the dedupe. Running it before would
		// let the more recently touched copy win, quietly replacing the
		// primary-provider rule with a recency one.
		const shared = 42;
		const rows: Partial<Record<Provider, ListEntry[]>> = {
			anilist: [
				entry({
					provider: 'anilist',
					media_id: 9001,
					mal_id: shared,
					updated_at_epoch_s: 1_700_000_000,
					title: 'AniList copy'
				})
			],
			mal: [
				entry({
					provider: 'mal',
					media_id: shared,
					mal_id: shared,
					updated_at_epoch_s: 1_700_009_999,
					title: 'MAL copy'
				})
			]
		};
		const out = mergedWatchLater(rows, 'anilist');
		expect(out).toHaveLength(1);
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
