import { describe, expect, it } from 'vitest';
import { followPlayed, playedEpisode } from './played-episode';
import { readPosition, savePosition, type PositionStorage } from './watch-position';

function memory(): PositionStorage {
	const data = new Map<string, string>();
	return {
		getItem: (k) => data.get(k) ?? null,
		setItem: (k, v) => void data.set(k, v)
	};
}

describe('playedEpisode', () => {
	it('is the episode the session names', () => {
		// Asked for by the number an earlier version gave it, the
		// stitched recap plays as the episode it is now.
		expect(playedEpisode({ episode: '12.5' }, 24.5)).toBe(12.5);
	});

	it('is the request where the session names none or nothing readable', () => {
		expect(playedEpisode({}, 7)).toBe(7);
		expect(playedEpisode({ episode: 'special' }, 7)).toBe(7);
	});
});

describe('followPlayed', () => {
	it('moves the position kept under the number asked for to the episode played', () => {
		const s = memory();
		savePosition('k1', 24.5, 300, 1400, s);
		expect(followPlayed('k1', { episode: '12.5' }, 24.5, s)).toBe(12.5);
		expect(readPosition('k1', 12.5, s)).toBe(300);
		expect(readPosition('k1', 24.5, s)).toBeNull();
	});

	it('moves the position kept under the number an earlier version gave the episode', () => {
		// Asked for as it is numbered now, the recap's position from
		// before is under the number it had then.
		const s = memory();
		savePosition('k1', 24.5, 300, 1400, s);
		expect(followPlayed('k1', { episode: '12.5', former_episode: '24.5' }, 12.5, s)).toBe(12.5);
		expect(readPosition('k1', 12.5, s)).toBe(300);
		expect(readPosition('k1', 24.5, s)).toBeNull();
	});

	it('leaves every position alone when the session plays what was asked for', () => {
		const s = memory();
		savePosition('k1', 5, 300, 1400, s);
		savePosition('k1', 6, 200, 1400, s);
		expect(followPlayed('k1', { episode: '5' }, 5, s)).toBe(5);
		expect(readPosition('k1', 5, s)).toBe(300);
		expect(readPosition('k1', 6, s)).toBe(200);
	});
});
