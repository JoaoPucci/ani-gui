// @vitest-environment happy-dom
//
// The orchestration wires real listeners on a real element, so the
// specs run against happy-dom and the module's actual collaborators
// (the shared machine, carrier and cleanup registry), reset around
// each case.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { armSourceScopedListeners } from './arm-source-listeners';
import { recoveryResume } from './resume-after-recovery';
import { stallMachine } from './stall-machine';
import { flushSourceScopedCleanups } from './global-video';

const hostSlow = {
	err: { source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' } as const,
	hasAutoRetried: false
};

let video: HTMLVideoElement;

beforeEach(() => {
	stallMachine.reset();
	recoveryResume.consume('drain', 0);
	video = document.createElement('video');
});

afterEach(() => {
	flushSourceScopedCleanups();
	stallMachine.reset();
});

describe('armSourceScopedListeners', () => {
	it('seeks back on metadata when a matching capture is pending', () => {
		recoveryResume.capture('show-a', 6, 432.5);
		armSourceScopedListeners({ video, showId: 'show-a', episode: 6 });
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		expect(video.currentTime).toBe(432.5);
	});

	it('arms no seek without a capture, and consumes mismatches', () => {
		recoveryResume.capture('show-a', 6, 432.5);
		armSourceScopedListeners({ video, showId: 'show-b', episode: 6 });
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		expect(video.currentTime).toBe(0);
		// The mismatch consumed the capture — it cannot leak forward.
		expect(recoveryResume.consume('show-a', 6)).toBeNull();
	});

	it('marks the machine proven once playback crosses the threshold', () => {
		armSourceScopedListeners({ video, showId: 'show-a', episode: 6 });
		video.currentTime = 0.4;
		video.dispatchEvent(new Event('timeupdate'));
		expect(stallMachine.failure(hostSlow)).toEqual({ act: 'recover' });
		stallMachine.reset();
		video.currentTime = 300;
		video.dispatchEvent(new Event('timeupdate'));
		expect(stallMachine.failure(hostSlow)).toEqual({ act: 'nudge', toast: true });
	});

	it('the next attach flushes the previous listeners away', () => {
		// The attach path flushes the source-scoped cleanups before
		// arming its own — the registry is additive now, because a
		// source owns more than one cleanup (its engine too).
		recoveryResume.capture('show-a', 6, 432.5);
		armSourceScopedListeners({ video, showId: 'show-a', episode: 6 });
		flushSourceScopedCleanups();
		armSourceScopedListeners({ video, showId: 'show-a', episode: 7 });
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		expect(video.currentTime).toBe(0);
	});
});
