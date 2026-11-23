// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import { holdForResume } from './resume-hold';

describe('holdForResume — once it has ended', () => {
	it('a release afterwards starts nothing: an ended hold has handed the element back', () => {
		const video = document.createElement('video');
		const play = vi.fn(() => Promise.resolve());
		video.play = play as unknown as HTMLVideoElement['play'];
		const hold = holdForResume({ video, onHold: () => {} });
		hold.end();
		hold.release();
		expect(play).not.toHaveBeenCalled();
	});

	it('a seek afterwards still moves the playhead, and its landing starts nothing', () => {
		const video = document.createElement('video');
		const play = vi.fn(() => Promise.resolve());
		video.play = play as unknown as HTMLVideoElement['play'];
		const hold = holdForResume({ video, onHold: () => {} });
		hold.end();
		hold.seekTo(42);
		expect(video.currentTime).toBe(42);
		video.dispatchEvent(new Event('seeked'));
		expect(play).not.toHaveBeenCalled();
	});
});
