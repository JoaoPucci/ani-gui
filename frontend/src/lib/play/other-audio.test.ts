import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AvailabilityBatchResponse } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import {
	describePlayFailureFromCache,
	lookupOtherAudio,
	otherAudioForFailure,
	otherAudioMode
} from './other-audio';

const EPISODE_UNAVAILABLE = {
	kind: 'episode_unavailable',
	key: 'error.play.episode_unavailable'
};

function batchAnswering(res: AvailabilityBatchResponse) {
	return vi.fn<(ids: string[], mode: string) => Promise<AvailabilityBatchResponse>>(
		async () => res
	);
}

afterEach(() => {
	vi.useRealTimers();
});

describe('otherAudioMode', () => {
	it('is the audio the user is not playing', () => {
		expect(otherAudioMode('sub')).toBe('dub');
		expect(otherAudioMode('dub')).toBe('sub');
		// The play path reads anything but "dub" as sub.
		expect(otherAudioMode('')).toBe('dub');
	});
});

describe('lookupOtherAudio — what the availability cache says, never a probe', () => {
	it('asks the cache for the other audio of this show only', async () => {
		const batch = batchAnswering({ cached: { '40684': false }, playable_episode_counts: {} });
		await lookupOtherAudio({ kitsuId: '40684', mode: 'sub', episode: 5 }, batch);
		expect(batch).toHaveBeenCalledTimes(1);
		expect(batch).toHaveBeenCalledWith(['40684'], 'dub');
	});

	it('is true when the cache has the other audio and its count covers the episode', async () => {
		const batch = batchAnswering({
			cached: { '7': true },
			playable_episode_counts: { '7': 12 }
		});
		expect(await lookupOtherAudio({ kitsuId: '7', mode: 'dub', episode: 12 }, batch)).toBe(true);
		expect(batch).toHaveBeenCalledWith(['7'], 'sub');
	});

	it('is unknown when the cache has the other audio without a count', async () => {
		// A count-less positive row (a dub resolve stamps one) proves
		// the show is there, not that this episode is: no evidence for
		// the suggestion.
		const batch = batchAnswering({ cached: { '7': true }, playable_episode_counts: {} });
		expect(await lookupOtherAudio({ kitsuId: '7', mode: 'sub', episode: 30 }, batch)).toBeNull();
	});

	it('is false when the other audio carries fewer episodes than this one', async () => {
		// The other audio's pages gate episodes by the cached count, so
		// an episode past it would be disabled there.
		const batch = batchAnswering({
			cached: { '7': true },
			playable_episode_counts: { '7': 4 }
		});
		expect(await lookupOtherAudio({ kitsuId: '7', mode: 'sub', episode: 5 }, batch)).toBe(false);
	});

	it('is false when the cache knows the other audio has nothing', async () => {
		// Kitsu 40684: sub cached with four episodes, dub cached as
		// unavailable.
		const batch = batchAnswering({ cached: { '40684': false }, playable_episode_counts: {} });
		expect(await lookupOtherAudio({ kitsuId: '40684', mode: 'sub', episode: 1 }, batch)).toBe(
			false
		);
	});

	it('is unknown when the cache holds no answer for the other audio', async () => {
		// Never probed, or a row the backend will not serve (unwalked,
		// or a negative its provider cannot back): absent from the map.
		const batch = batchAnswering({ cached: {}, playable_episode_counts: {} });
		expect(await lookupOtherAudio({ kitsuId: '7', mode: 'sub', episode: 1 }, batch)).toBeNull();
	});

	it('is unknown when the lookup fails', async () => {
		const batch = vi.fn(async () => {
			throw new Error('sidecar gone');
		});
		expect(await lookupOtherAudio({ kitsuId: '7', mode: 'sub', episode: 1 }, batch)).toBeNull();
	});

	it('is unknown when the lookup answers nonsense', async () => {
		const batch = vi.fn(async () => null as unknown as AvailabilityBatchResponse);
		expect(await lookupOtherAudio({ kitsuId: '7', mode: 'sub', episode: 1 }, batch)).toBeNull();
	});

	it('is unknown, without waiting on it, when the lookup hangs', async () => {
		vi.useFakeTimers();
		const batch = vi.fn(() => new Promise<AvailabilityBatchResponse>(() => {}));
		const answer = lookupOtherAudio({ kitsuId: '7', mode: 'sub', episode: 1 }, batch);
		await vi.advanceTimersByTimeAsync(5000);
		expect(await answer).toBeNull();
	});

	it('is unknown, and asks nothing, without a show id', async () => {
		const batch = batchAnswering({ cached: {}, playable_episode_counts: {} });
		expect(await lookupOtherAudio({ kitsuId: null, mode: 'sub', episode: 1 }, batch)).toBeNull();
		expect(await lookupOtherAudio({ kitsuId: '', mode: 'sub', episode: 1 }, batch)).toBeNull();
		expect(batch).not.toHaveBeenCalled();
	});

	it('is unknown when no episode is named', async () => {
		// A range download does not say which of its episodes failed,
		// so no count can vouch for it.
		const batch = batchAnswering({
			cached: { '7': true },
			playable_episode_counts: { '7': 4 }
		});
		expect(await lookupOtherAudio({ kitsuId: '7', mode: 'sub', episode: null }, batch)).toBeNull();
	});

	it('still knows the other audio has nothing when no episode is named', async () => {
		const batch = batchAnswering({ cached: { '7': false }, playable_episode_counts: {} });
		expect(await lookupOtherAudio({ kitsuId: '7', mode: 'sub', episode: null }, batch)).toBe(false);
	});
});

describe('otherAudioForFailure', () => {
	it('asks only about an episode missing from this audio', async () => {
		const batch = batchAnswering({ cached: { '7': true }, playable_episode_counts: { '7': 12 } });
		for (const e of [{ kind: 'timeout' }, { kind: 'no_results' }, new Error('x'), null]) {
			expect(await otherAudioForFailure(e, { kitsuId: '7', mode: 'sub', episode: 1 }, batch)).toBe(
				null
			);
		}
		expect(batch).not.toHaveBeenCalled();
		expect(
			await otherAudioForFailure(
				EPISODE_UNAVAILABLE,
				{ kitsuId: '7', mode: 'sub', episode: 1 },
				batch
			)
		).toBe(true);
		expect(batch).toHaveBeenCalledTimes(1);
	});
});

describe('describePlayFailureFromCache', () => {
	const q = { kitsuId: '7', mode: 'sub', episode: 3 };

	it('suggests the other audio only when the cache vouches for it', async () => {
		const yes = batchAnswering({ cached: { '7': true }, playable_episode_counts: { '7': 12 } });
		const no = batchAnswering({ cached: { '7': false }, playable_episode_counts: {} });
		const unknown = batchAnswering({ cached: {}, playable_episode_counts: {} });
		expect(await describePlayFailureFromCache(EPISODE_UNAVAILABLE, q, {}, yes)).toBe(
			m.play_play_failure_episode_unavailable_try_other_audio()
		);
		expect(await describePlayFailureFromCache(EPISODE_UNAVAILABLE, q, {}, no)).toBe(
			m.play_play_failure_episode_unavailable()
		);
		expect(await describePlayFailureFromCache(EPISODE_UNAVAILABLE, q, {}, unknown)).toBe(
			m.play_play_failure_episode_unavailable()
		);
	});

	it("keeps a surface's catalogue-miss phrasing", async () => {
		const batch = batchAnswering({ cached: {}, playable_episode_counts: {} });
		expect(
			await describePlayFailureFromCache(
				{ kind: 'no_results' },
				q,
				{ noResults: () => 'NOT IN CATALOGUE' },
				batch
			)
		).toBe('NOT IN CATALOGUE');
		expect(batch).not.toHaveBeenCalled();
	});
});
