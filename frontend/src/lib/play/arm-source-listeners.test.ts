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

	it('marks the machine proven when playback actually starts', () => {
		// The signal is the element's `playing` event — frames
		// rendered, so on a fresh source the network delivered. A
		// timeupdate alone proves nothing: the resume seek emits one
		// at the old timestamp before the new stream has produced a
		// single frame. Contract sharpened in this red: the previous
		// spec proved via timeupdate and let exactly that false
		// positive through.
		armSourceScopedListeners({ video, showId: 'show-a', episode: 6 });
		video.currentTime = 300;
		video.dispatchEvent(new Event('timeupdate'));
		expect(stallMachine.failure(hostSlow)).toEqual({ act: 'recover' });
		stallMachine.reset();
		video.dispatchEvent(new Event('playing'));
		expect(stallMachine.failure(hostSlow)).toEqual({ act: 'nudge', toast: true });
	});

	it('the resume seek does not prove the fresh stream', () => {
		// Recovery resumes at minutes in; the seek assigns the old
		// timestamp and the element fires timeupdate. If the
		// replacement URL is immediately dead, its stall must take
		// the startup path — recover — not spend nudges the stream
		// never earned.
		recoveryResume.capture('show-a', 6, 432.5);
		armSourceScopedListeners({ video, showId: 'show-a', episode: 6 });
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		expect(video.currentTime).toBe(432.5);
		video.dispatchEvent(new Event('timeupdate'));
		expect(stallMachine.failure(hostSlow)).toEqual({ act: 'recover' });
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
