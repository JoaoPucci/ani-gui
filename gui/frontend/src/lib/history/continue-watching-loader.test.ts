import { describe, expect, it, vi } from 'vitest';
import { loadContinueWatchingState } from './continue-watching-loader';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

function makeEntry(id: string, ep: string, title: string): HistoryEntry {
	return { id, ep_no: ep, title };
}

function makeMatch(id: string, episodeCount: number | null): KitsuAnimeRef {
	return {
		id,
		slug: `slug-${id}`,
		canonical_title: `Title ${id}`,
		titles: {},
		episode_count: episodeCount,
		subtype: 'TV',
		status: 'current',
		poster_image: null,
		start_date: null
	} as unknown as KitsuAnimeRef;
}

// Defer helper — returns a promise and its resolver so the test
// can sequence resolve order across stages.
function defer<T>(): { promise: Promise<T>; resolve: (v: T) => void } {
	let resolveFn!: (v: T) => void;
	const promise = new Promise<T>((res) => {
		resolveFn = res;
	});
	return { promise, resolve: resolveFn };
}

describe('loadContinueWatchingState', () => {
	it('does not resolve until the batch availability call settles', async () => {
		// Codex P2 race: matches arriving per-entry should not flip the
		// card to "resumable" before the playable count is in, because
		// the click would derive nextEpisode from match.episode_count
		// (Kitsu's possibly-stale cap) and replay the last episode for
		// ongoing shows where allmanga has newer episodes.
		const entry = makeEntry('one-piece', '1100', 'One Piece');
		const match = makeMatch('kitsu-12', 1100); // Kitsu's stale cap
		const matchDeferred = defer<KitsuAnimeRef | null>();
		const batchDeferred = defer<{ playable_episode_counts: Record<string, number> }>();

		const resolveMatch = vi.fn(() => matchDeferred.promise);
		const fetchAvailabilityBatch = vi.fn(() => batchDeferred.promise);

		let resolved = false;
		const loaderPromise = loadContinueWatchingState([entry], {
			resolveMatch,
			fetchAvailabilityBatch,
			mode: 'sub'
		}).then((r) => {
			resolved = true;
			return r;
		});

		// Per-entry match arrives — the loader should NOT have
		// resolved yet because the batch is still pending.
		matchDeferred.resolve(match);
		await Promise.resolve();
		await Promise.resolve();
		expect(resolved).toBe(false);

		// Batch arrives — now the loader resolves with both maps.
		batchDeferred.resolve({ playable_episode_counts: { 'kitsu-12': 1107 } });
		const result = await loaderPromise;
		expect(result.matches).toEqual({ 'one-piece': match });
		expect(result.playableCounts).toEqual({ 'one-piece': 1107 });
	});

	it('keys matches by HistoryEntry.id and surfaces playable counts via kitsu id', async () => {
		const e1 = makeEntry('hist-a', '5', 'Show A');
		const e2 = makeEntry('hist-b', '2', 'Show B');
		const m1 = makeMatch('k-a', 12);
		const m2 = makeMatch('k-b', 24);

		const resolveMatch = vi.fn().mockImplementation((entry: HistoryEntry) => {
			if (entry.id === 'hist-a') return Promise.resolve(m1);
			if (entry.id === 'hist-b') return Promise.resolve(m2);
			return Promise.resolve(null);
		});
		const fetchAvailabilityBatch = vi
			.fn()
			.mockResolvedValue({ playable_episode_counts: { 'k-a': 15, 'k-b': 24 } });

		const result = await loadContinueWatchingState([e1, e2], {
			resolveMatch,
			fetchAvailabilityBatch,
			mode: 'sub'
		});

		expect(fetchAvailabilityBatch).toHaveBeenCalledWith(['k-a', 'k-b'], 'sub');
		expect(result.matches).toEqual({ 'hist-a': m1, 'hist-b': m2 });
		expect(result.playableCounts).toEqual({ 'hist-a': 15, 'hist-b': 24 });
	});

	it('handles a no-match entry without throwing and keeps it out of playableCounts', async () => {
		const e1 = makeEntry('hist-a', '5', 'Show A');
		const e2 = makeEntry('hist-orphan', '1', 'Unmatchable');
		const m1 = makeMatch('k-a', 12);

		const resolveMatch = vi.fn().mockImplementation((entry: HistoryEntry) => {
			if (entry.id === 'hist-a') return Promise.resolve(m1);
			return Promise.resolve(null);
		});
		const fetchAvailabilityBatch = vi
			.fn()
			.mockResolvedValue({ playable_episode_counts: { 'k-a': 15 } });

		const result = await loadContinueWatchingState([e1, e2], {
			resolveMatch,
			fetchAvailabilityBatch,
			mode: 'sub'
		});

		expect(result.matches).toEqual({ 'hist-a': m1, 'hist-orphan': null });
		expect(result.playableCounts).toEqual({ 'hist-a': 15 });
	});

	it('skips the batch call entirely when no matches resolve', async () => {
		const e1 = makeEntry('hist-orphan', '1', 'Unmatchable');
		const resolveMatch = vi.fn().mockResolvedValue(null);
		const fetchAvailabilityBatch = vi.fn();

		const result = await loadContinueWatchingState([e1], {
			resolveMatch,
			fetchAvailabilityBatch,
			mode: 'sub'
		});

		expect(fetchAvailabilityBatch).not.toHaveBeenCalled();
		expect(result.matches).toEqual({ 'hist-orphan': null });
		expect(result.playableCounts).toEqual({});
	});

	it('treats a per-entry resolveMatch rejection as null match (no throw)', async () => {
		const e1 = makeEntry('hist-a', '5', 'Show A');
		const e2 = makeEntry('hist-b', '2', 'Show B');
		const m2 = makeMatch('k-b', 24);

		const resolveMatch = vi.fn().mockImplementation((entry: HistoryEntry) => {
			if (entry.id === 'hist-a') return Promise.reject(new Error('boom'));
			return Promise.resolve(m2);
		});
		const fetchAvailabilityBatch = vi
			.fn()
			.mockResolvedValue({ playable_episode_counts: { 'k-b': 24 } });

		const result = await loadContinueWatchingState([e1, e2], {
			resolveMatch,
			fetchAvailabilityBatch,
			mode: 'sub'
		});

		expect(result.matches).toEqual({ 'hist-a': null, 'hist-b': m2 });
		expect(result.playableCounts).toEqual({ 'hist-b': 24 });
	});

	it('falls back to empty playableCounts when the batch rejects', async () => {
		// Cache miss or network blip on the batch endpoint: the page
		// should still render Continue cards (with Kitsu-cap fallback)
		// rather than stay stuck in the loading state forever.
		const e1 = makeEntry('hist-a', '5', 'Show A');
		const m1 = makeMatch('k-a', 12);

		const resolveMatch = vi.fn().mockResolvedValue(m1);
		const fetchAvailabilityBatch = vi.fn().mockRejectedValue(new Error('cache miss'));

		const result = await loadContinueWatchingState([e1], {
			resolveMatch,
			fetchAvailabilityBatch,
			mode: 'sub'
		});

		expect(result.matches).toEqual({ 'hist-a': m1 });
		expect(result.playableCounts).toEqual({});
	});
});
