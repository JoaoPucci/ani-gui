import { describe, expect, it, vi } from 'vitest';
import { makeFetchAvailability } from './availability-from-match';
import type { AvailabilityArgs, AvailabilityResponse, KitsuAnimeRef } from '$lib/api';

function makeMatch(overrides: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id: 'kitsu-id',
		slug: 'slug',
		canonical_title: 'Canonical Title',
		titles: { en_jp: 'Canonical Title', ja_jp: '正準題', en: 'English Title' },
		episode_count: 12,
		subtype: 'TV',
		status: 'current',
		poster_image: null,
		start_date: '2024-01-12',
		...overrides
	} as unknown as KitsuAnimeRef;
}

describe('makeFetchAvailability', () => {
	it('maps a match into AvailabilityArgs and forwards to the IPC fn', async () => {
		const ipc = vi.fn().mockResolvedValue({
			available: true,
			episode_count: 13,
			extra_episodes: []
		} satisfies AvailabilityResponse);
		const fetcher = makeFetchAvailability(ipc);
		const match = makeMatch();

		const result = await fetcher(match, 'sub');
		expect(ipc).toHaveBeenCalledTimes(1);
		const call = ipc.mock.calls[0][0] as AvailabilityArgs;
		expect(call.title).toBe('Canonical Title');
		expect(call.mode).toBe('sub');
		expect(call.kitsu_id).toBe('kitsu-id');
		expect(call.episode_count).toBe(12);
		expect(call.year).toBe(2024);
		expect(call.status).toBe('current');
		expect(result.episode_count).toBe(13);
	});

	it('omits episode_count, year, and status when the match has none', async () => {
		// AvailabilityArgs's optional fields use `?: T` (not `T | null`),
		// so the helper must drop null/undefined entirely rather than
		// pass them through as `null` — the backend's title-match
		// disambiguation differentiates "no signal" from "explicit
		// zero/empty," and a `null` would round-trip as the wrong
		// signal.
		const ipc = vi.fn().mockResolvedValue({
			available: false,
			episode_count: null,
			extra_episodes: []
		} satisfies AvailabilityResponse);
		const fetcher = makeFetchAvailability(ipc);
		const match = makeMatch({
			episode_count: null,
			status: null,
			start_date: null
		} as Partial<KitsuAnimeRef>);

		await fetcher(match, 'dub');
		const call = ipc.mock.calls[0][0] as AvailabilityArgs;
		expect(call.mode).toBe('dub');
		expect(call.episode_count).toBeUndefined();
		expect(call.year).toBeUndefined();
		expect(call.status).toBeUndefined();
	});
});
