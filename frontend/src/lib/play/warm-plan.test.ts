import { describe, expect, it } from 'vitest';
import { planWarm } from './warm-plan';

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
