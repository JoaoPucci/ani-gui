// @vitest-environment happy-dom
//
// The orchestration wires real listeners on a real element, so the
// specs run against happy-dom and the module's actual collaborators
// (the shared machine, the recovery carrier, a page's source scope
// and a position store), reset around each case.
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { armSourceScopedListeners } from './arm-source-listeners';
import { recoveryResume } from './resume-after-recovery';
import { stallMachine } from './stall-machine';
import { createSourceScope, type SourceScope } from './source-scope';
import { readPosition, savePosition, type PositionStorage } from './watch-position';

const hostSlow = {
	err: { source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' } as const,
	hasAutoRetried: false
};

let video: HTMLVideoElement;
let scope: SourceScope;
let positions: PositionStorage;

function arm(showId: string, episode: number) {
	armSourceScopedListeners({ video, showId, episode, scope, positions });
}

function playAt(seconds: number, duration = 1420) {
	Object.defineProperty(video, 'duration', { configurable: true, get: () => duration });
	video.currentTime = seconds;
}

beforeEach(() => {
	stallMachine.reset();
	recoveryResume.consume('drain', 0);
	video = document.createElement('video');
	scope = createSourceScope();
	const data = new Map<string, string>();
	positions = { getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
});

afterEach(() => {
	scope.flush();
	stallMachine.reset();
});

describe('armSourceScopedListeners', () => {
	it('seeks back on metadata when a matching capture is pending', () => {
		recoveryResume.capture('show-a', 6, 432.5);
		arm('show-a', 6);
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		expect(video.currentTime).toBe(432.5);
	});

	it('arms no seek without a capture, and consumes mismatches', () => {
		recoveryResume.capture('show-a', 6, 432.5);
		arm('show-b', 6);
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
		arm('show-a', 6);
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
		arm('show-a', 6);
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		expect(video.currentTime).toBe(432.5);
		video.dispatchEvent(new Event('timeupdate'));
		expect(stallMachine.failure(hostSlow)).toEqual({ act: 'recover' });
	});

	it('the next attach flushes the previous listeners away', () => {
		// The attach path flushes the page's source scope before
		// arming its own — the scope is additive, because a source
		// owns more than one cleanup (its engine too).
		recoveryResume.capture('show-a', 6, 432.5);
		arm('show-a', 6);
		scope.flush();
		arm('show-a', 7);
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		expect(video.currentTime).toBe(0);
	});

	it('seeks to where the episode was left when no recovery is pending', () => {
		savePosition('show-a', 6, 612.5, 1420, positions);
		arm('show-a', 6);
		playAt(0);
		video.dispatchEvent(new Event('loadedmetadata'));
		expect(video.currentTime).toBe(612.5);
	});

	it("a pending recovery's position wins over the saved one", () => {
		savePosition('show-a', 6, 612.5, 1420, positions);
		recoveryResume.capture('show-a', 6, 700);
		arm('show-a', 6);
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		expect(video.currentTime).toBe(700);
	});

	it('remembers where playback is as it plays, and where it was left', () => {
		arm('show-a', 6);
		video.dispatchEvent(new Event('loadedmetadata'));
		playAt(300);
		video.dispatchEvent(new Event('timeupdate'));
		expect(readPosition('show-a', 6, positions)).toBe(300);
		playAt(302);
		video.dispatchEvent(new Event('timeupdate'));
		// Not written on every tick.
		expect(readPosition('show-a', 6, positions)).toBe(300);
		playAt(303.5);
		video.dispatchEvent(new Event('pause'));
		expect(readPosition('show-a', 6, positions)).toBe(303.5);
		playAt(310);
		scope.flush();
		expect(readPosition('show-a', 6, positions)).toBe(310);
	});

	it('a source left before its metadata loaded keeps the saved position', () => {
		// Leaving while the stream is still opening has the element at
		// zero; writing that would forget where the episode was left.
		savePosition('show-a', 6, 612.5, 1420, positions);
		arm('show-a', 6);
		video.currentTime = 0;
		scope.flush();
		expect(readPosition('show-a', 6, positions)).toBe(612.5);
	});

	it('an episode played to its end is forgotten', () => {
		savePosition('show-a', 6, 612.5, 1420, positions);
		arm('show-a', 6);
		video.dispatchEvent(new Event('loadedmetadata'));
		playAt(1420);
		video.dispatchEvent(new Event('ended'));
		expect(readPosition('show-a', 6, positions)).toBeNull();
	});

	it('a flushed source writes nothing more', () => {
		arm('show-a', 6);
		video.dispatchEvent(new Event('loadedmetadata'));
		playAt(300);
		scope.flush();
		playAt(900);
		video.dispatchEvent(new Event('pause'));
		expect(readPosition('show-a', 6, positions)).toBe(300);
	});

	describe('a kept point saved while the length was unknown', () => {
		// A point saved while the stream's length was not known escapes
		// the finished cutoff. The visit that learns the length is the
		// first that can apply it: a point in the last 90 seconds is
		// finished, so it is forgotten and not sought to.
		function metadataWith(duration: number) {
			Object.defineProperty(video, 'duration', { configurable: true, get: () => duration });
			video.currentTime = 0;
			video.dispatchEvent(new Event('loadedmetadata'));
		}

		it('is forgotten, not sought to, once the length shows it in the last 90 seconds', () => {
			savePosition('show-a', 6, 1400, Number.NaN, positions);
			arm('show-a', 6);
			metadataWith(1420);
			expect(video.currentTime).toBe(0);
			expect(readPosition('show-a', 6, positions)).toBeNull();
		});

		it('is sought to when the length shows it short of the end', () => {
			savePosition('show-a', 6, 600, Number.NaN, positions);
			arm('show-a', 6);
			metadataWith(1420);
			expect(video.currentTime).toBe(600);
			expect(readPosition('show-a', 6, positions)).toBe(600);
		});

		function durationBecomes(duration: number) {
			Object.defineProperty(video, 'duration', { configurable: true, get: () => duration });
			video.dispatchEvent(new Event('durationchange'));
		}

		it('waits for a known length before seeking, then seeks to a point short of the end', () => {
			savePosition('show-a', 6, 600, Number.NaN, positions);
			arm('show-a', 6);
			metadataWith(Number.POSITIVE_INFINITY);
			expect(video.currentTime).toBe(0);
			durationBecomes(Number.NaN);
			expect(video.currentTime).toBe(0);
			durationBecomes(1420);
			expect(video.currentTime).toBe(600);
		});

		it('waits for a known length, then forgets a point in the last 90 seconds', () => {
			savePosition('show-a', 6, 1400, Number.NaN, positions);
			arm('show-a', 6);
			metadataWith(Number.POSITIVE_INFINITY);
			expect(video.currentTime).toBe(0);
			durationBecomes(1420);
			expect(video.currentTime).toBe(0);
			expect(readPosition('show-a', 6, positions)).toBeNull();
		});

		it('decides once: a later length change moves nothing', () => {
			savePosition('show-a', 6, 600, Number.NaN, positions);
			arm('show-a', 6);
			metadataWith(1420);
			video.currentTime = 700;
			durationBecomes(1425);
			expect(video.currentTime).toBe(700);
		});

		it('autoplay while waiting neither drops the resume nor writes over the kept point', () => {
			// The element autoplays: it starts playing, and ticks, before
			// the length is known. Neither is the viewer moving.
			savePosition('show-a', 6, 600, Number.NaN, positions);
			arm('show-a', 6);
			metadataWith(Number.POSITIVE_INFINITY);
			video.dispatchEvent(new Event('playing'));
			video.currentTime = 0.3;
			video.dispatchEvent(new Event('timeupdate'));
			video.dispatchEvent(new Event('pause'));
			expect(readPosition('show-a', 6, positions)).toBe(600);
			durationBecomes(1420);
			expect(video.currentTime).toBe(600);
		});

		it("the engine's own seek while waiting does not drop the resume", () => {
			// hls.js jumps gaps and the stream's start by seeking the
			// element; that is not the viewer moving.
			savePosition('show-a', 6, 600, Number.NaN, positions);
			arm('show-a', 6);
			metadataWith(Number.POSITIVE_INFINITY);
			video.currentTime = 0.2;
			video.dispatchEvent(new Event('seeking'));
			durationBecomes(1420);
			expect(video.currentTime).toBe(600);
		});

		it('leaving while waiting keeps the kept point', () => {
			// The element is still at zero while it waits; writing that
			// would forget where the episode was left.
			savePosition('show-a', 6, 600, Number.NaN, positions);
			arm('show-a', 6);
			metadataWith(Number.POSITIVE_INFINITY);
			scope.flush();
			expect(readPosition('show-a', 6, positions)).toBe(600);
		});

		it('a length that never becomes known never seeks, and nothing is written for the stream', () => {
			savePosition('show-a', 6, 1400, Number.NaN, positions);
			arm('show-a', 6);
			metadataWith(Number.POSITIVE_INFINITY);
			video.dispatchEvent(new Event('playing'));
			video.currentTime = 40;
			video.dispatchEvent(new Event('timeupdate'));
			video.dispatchEvent(new Event('pause'));
			scope.flush();
			expect(readPosition('show-a', 6, positions)).toBe(1400);
		});

		it("a pending recovery's point is sought to wherever it falls", () => {
			// A recovery resumes the stream the viewer was watching a
			// moment ago; the cutoff is about what a later visit opens.
			recoveryResume.capture('show-a', 6, 1400);
			arm('show-a', 6);
			metadataWith(1420);
			expect(video.currentTime).toBe(1400);
		});
	});
});
