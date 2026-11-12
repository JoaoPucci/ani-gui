// Playback held while the change-show picker is open over the player.

import { describe, expect, it, vi } from 'vitest';
import fc from 'fast-check';
import { holdPlayback, type Playable } from './hold-playback';

function media(playing: boolean) {
	const m = {
		paused: !playing,
		pause: vi.fn(() => {
			m.paused = true;
		}),
		play: vi.fn(async () => {
			m.paused = false;
		})
	};
	return m satisfies Playable;
}

describe('holdPlayback', () => {
	it('pauses a playing video, and plays it again on release', () => {
		const v = media(true);
		const release = holdPlayback(v);
		expect(v.pause).toHaveBeenCalledTimes(1);
		expect(v.paused).toBe(true);
		release();
		expect(v.play).toHaveBeenCalledTimes(1);
	});

	it('leaves a paused video paused, then and on release', () => {
		const v = media(false);
		holdPlayback(v)();
		expect(v.pause).not.toHaveBeenCalled();
		expect(v.play).not.toHaveBeenCalled();
	});

	it('does not play a video the viewer started again meanwhile', () => {
		const v = media(true);
		const release = holdPlayback(v);
		v.paused = false;
		release();
		expect(v.play).not.toHaveBeenCalled();
	});

	it('holds nothing without a video', () => {
		expect(() => holdPlayback(null)()).not.toThrow();
		expect(() => holdPlayback(undefined)()).not.toThrow();
	});

	it('swallows a refused play', async () => {
		const v = media(true);
		v.play.mockImplementation(() => Promise.reject(new Error('NotAllowedError')));
		holdPlayback(v)();
		await Promise.resolve();
		expect(v.play).toHaveBeenCalledTimes(1);
	});

	it('plays at most once, and only if it was playing, however often it is released', () => {
		fc.assert(
			fc.property(fc.boolean(), fc.integer({ min: 1, max: 5 }), (playing, releases) => {
				const v = media(playing);
				const release = holdPlayback(v);
				for (let i = 0; i < releases; i++) release();
				expect(v.play).toHaveBeenCalledTimes(playing ? 1 : 0);
				expect(v.paused).toBe(!playing);
			})
		);
	});
});
