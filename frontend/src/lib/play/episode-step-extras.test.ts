import { describe, it, expect } from 'vitest';
import { extraNumbers, nextEpisode, previousEpisode, stepAllowed } from './episode-number';
import { decideAutoPlayNext } from './auto-play-next';

// Satsuriku no Tenshi's four web-only episodes follow its twelve as
// the extras 12.1 to 12.4, and they are the story's ending: stepping
// from one extra goes to the next extra beside the same episode before
// it goes on to the next whole episode.
const SATSURIKU = [12.1, 12.2, 12.3, 12.4];

describe('stepping among the extras beside one episode', () => {
	it('steps from an extra to the next extra beside the same episode', () => {
		expect(nextEpisode(12.1, SATSURIKU)).toBe(12.2);
		expect(nextEpisode(12.3, SATSURIKU)).toBe(12.4);
		expect(previousEpisode(12.2, SATSURIKU)).toBe(12.1);
		expect(previousEpisode(12.4, SATSURIKU)).toBe(12.3);
	});

	it('leaves the run to the whole episodes either side', () => {
		expect(nextEpisode(12.4, SATSURIKU)).toBe(13);
		expect(previousEpisode(12.1, SATSURIKU)).toBe(12);
		// A recap alone beside its episode steps as before.
		expect(nextEpisode(7.5, [7.5])).toBe(8);
		expect(previousEpisode(7.5, [7.5])).toBe(7);
		// Extras beside another episode are not this run.
		expect(nextEpisode(7.5, [7.5, 8.5])).toBe(8);
	});

	it('steps from a whole episode as before', () => {
		expect(nextEpisode(12, SATSURIKU)).toBe(13);
		expect(previousEpisode(13, SATSURIKU)).toBe(12);
		expect(nextEpisode(7, [7.5])).toBe(8);
	});

	it('lets a step past the cap land on a listed extra, and nowhere else', () => {
		expect(stepAllowed(12.2, 12, SATSURIKU)).toBe(true);
		expect(stepAllowed(13, 12, SATSURIKU)).toBe(false);
		expect(stepAllowed(12, 12, SATSURIKU)).toBe(true);
		expect(stepAllowed(99, null, [])).toBe(true);
	});

	it('auto-plays through the run and stops at its end', () => {
		const at = (episodeNum: number) =>
			decideAutoPlayNext({ enabled: true, episodeNum, totalEpisodes: 12, extras: SATSURIKU });
		expect(at(12.1)).toEqual({ advance: true, target: 12.2 });
		expect(at(12.3)).toEqual({ advance: true, target: 12.4 });
		expect(at(12.4)).toEqual({ advance: false });
		expect(at(12)).toEqual({ advance: false });
	});

	it('reads the listed tags as numbers', () => {
		expect(extraNumbers(['12.1', '12.2', 'x', '', '0'])).toEqual([12.1, 12.2]);
	});
});
