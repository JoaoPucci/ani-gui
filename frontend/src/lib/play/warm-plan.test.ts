import { describe, expect, it } from 'vitest';
import { detailWarmTargets, planWarm, playPageWarmTargets } from './warm-plan';
import type { AiringStatus } from '$lib/detail/episode-airing';

describe('planWarm', () => {
	it('keeps the wide fan-out when resolution caching is on', () => {
		// Opted-in caching makes revisit warms ~0.5s hits — the fan-out
		// earns its cost and every visible tile click stays instant.
		expect(planWarm({ cacheResolutions: true, candidates: [1, 2, 3, 4, 5], next: 2 })).toEqual([
			1, 2, 3, 4, 5
		]);
	});

	it('narrows to the single next target when caching is off', () => {
		// Every warm is a full multi-second walk now; only the episode
		// the user is most likely to play next is worth one.
		expect(planWarm({ cacheResolutions: false, candidates: [1, 2, 3, 4, 5], next: 2 })).toEqual([
			2
		]);
	});

	it('warms nothing when caching is off and there is no next target', () => {
		// Last episode of the series, or the next one is unaired /
		// beyond the playable cap.
		expect(planWarm({ cacheResolutions: false, candidates: [10, 11, 12], next: null })).toEqual([]);
	});

	it('a narrow target outside the visible candidates still warms', () => {
		// Play page: the current episode is the last tile of its strip
		// page, so current+1 is not among the visible candidates — the
		// warm is what makes auto-play across the boundary seamless.
		expect(planWarm({ cacheResolutions: false, candidates: [1, 2, 3, 4, 5], next: 6 })).toEqual([
			6
		]);
	});

	it('an empty candidate list stays empty in wide mode', () => {
		expect(planWarm({ cacheResolutions: true, candidates: [], next: null })).toEqual([]);
	});
});

const aired = (n: number): AiringStatus => ({
	aired: n,
	next_episode: n + 1,
	next_airing_at: null,
	upcoming: []
});

describe('playPageWarmTargets', () => {
	it('narrows to the validated next episode when caching is off', () => {
		expect(
			playPageWarmTargets({
				cacheResolutions: false,
				visible: [1, 2, 3, 4, 5],
				currentEpisode: 1,
				airing: aired(12),
				playableCount: 12
			})
		).toEqual([2]);
	});

	it('the next target crosses the strip-page boundary', () => {
		// Current episode is the last visible tile; ep 6 lives on the
		// next strip page and must still warm — auto-play lands there.
		expect(
			playPageWarmTargets({
				cacheResolutions: false,
				visible: [1, 2, 3, 4, 5],
				currentEpisode: 5,
				airing: aired(12),
				playableCount: 12
			})
		).toEqual([6]);
	});

	it('an unaired or cap-gated next episode warms nothing', () => {
		expect(
			playPageWarmTargets({
				cacheResolutions: false,
				visible: [1, 2, 3],
				currentEpisode: 3,
				airing: aired(3),
				playableCount: 12
			})
		).toEqual([]);
		expect(
			playPageWarmTargets({
				cacheResolutions: false,
				visible: [1, 2, 3],
				currentEpisode: 3,
				airing: aired(12),
				playableCount: 3
			})
		).toEqual([]);
	});

	it('wide mode keeps every aired, playable visible tile', () => {
		// Tile 5 is past the aired count and tile 4 past the playable
		// cap — both stay out; null tile numbers are dropped.
		expect(
			playPageWarmTargets({
				cacheResolutions: true,
				visible: [1, 2, null, 4, 5],
				currentEpisode: 1,
				airing: aired(4),
				playableCount: 3
			})
		).toEqual([1, 2]);
	});

	it('unknown airing leaves validation to the playable cap alone', () => {
		expect(
			playPageWarmTargets({
				cacheResolutions: false,
				visible: [1, 2],
				currentEpisode: 2,
				airing: null,
				playableCount: 12
			})
		).toEqual([3]);
	});
});

describe('detailWarmTargets', () => {
	it('narrows to the hero target when caching is off', () => {
		expect(
			detailWarmTargets({
				cacheResolutions: false,
				visible: [1, 2, 3, 4, 5],
				heroEpisode: 3,
				airing: aired(12),
				playableCount: 12
			})
		).toEqual([3]);
	});

	it('an unaired or cap-gated hero target warms nothing', () => {
		expect(
			detailWarmTargets({
				cacheResolutions: false,
				visible: [1, 2, 3],
				heroEpisode: 4,
				airing: aired(3),
				playableCount: 12
			})
		).toEqual([]);
		expect(
			detailWarmTargets({
				cacheResolutions: false,
				visible: [1, 2, 3],
				heroEpisode: 4,
				airing: aired(12),
				playableCount: 3
			})
		).toEqual([]);
	});

	it('wide mode fans out over the aired, playable grid tiles', () => {
		expect(
			detailWarmTargets({
				cacheResolutions: true,
				visible: [1, 2, null, 4, 5],
				heroEpisode: 1,
				airing: aired(4),
				playableCount: 4
			})
		).toEqual([1, 2, 4]);
	});

	it('the hero target stands in while the grid has not loaded', () => {
		expect(
			detailWarmTargets({
				cacheResolutions: true,
				visible: null,
				heroEpisode: 7,
				airing: aired(12),
				playableCount: 12
			})
		).toEqual([7]);
		expect(
			detailWarmTargets({
				cacheResolutions: false,
				visible: null,
				heroEpisode: 7,
				airing: aired(12),
				playableCount: 12
			})
		).toEqual([7]);
	});
});
