// @vitest-environment happy-dom
//
// The orchestration wires real listeners on a real element, so the
// specs run against happy-dom and the module's actual collaborators
// (the shared machine, the recovery carrier, a page's source scope
// and a position store), reset around each case.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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

	describe('an episode that played to its end', () => {
		// Ending forgets the episode; nothing the source does on its way
		// out — a pause, the flush when the next stream attaches or the
		// page leaves — may write it back, whatever the length says.
		function playToEnd(duration: number) {
			savePosition('show-a', 6, 612.5, Number.NaN, positions);
			arm('show-a', 6);
			Object.defineProperty(video, 'duration', { configurable: true, get: () => duration });
			video.currentTime = 0;
			video.dispatchEvent(new Event('loadedmetadata'));
			video.dispatchEvent(new Event('durationchange'));
			video.currentTime = Number.isFinite(duration) ? duration : 1420;
			video.dispatchEvent(new Event('ended'));
		}

		it('stays forgotten when its stream is left, with a known length', () => {
			playToEnd(1420);
			video.dispatchEvent(new Event('pause'));
			scope.flush();
			expect(readPosition('show-a', 6, positions)).toBeNull();
		});

		it('stays forgotten when its stream is left, with a length never known', () => {
			// No kept point to wait on: a started mark opens at once.
			arm('show-a', 6);
			Object.defineProperty(video, 'duration', {
				configurable: true,
				get: () => Number.POSITIVE_INFINITY
			});
			video.dispatchEvent(new Event('loadedmetadata'));
			video.currentTime = 1420;
			video.dispatchEvent(new Event('ended'));
			video.dispatchEvent(new Event('pause'));
			scope.flush();
			expect(readPosition('show-a', 6, positions)).toBeNull();
		});

		it('stays forgotten when the clip ends in its first seconds', () => {
			arm('show-a', 6);
			Object.defineProperty(video, 'duration', { configurable: true, get: () => 10 });
			video.dispatchEvent(new Event('loadedmetadata'));
			video.currentTime = 10;
			video.dispatchEvent(new Event('ended'));
			scope.flush();
			expect(readPosition('show-a', 6, positions)).toBeNull();
		});

		it('is kept again once playback resumes on the same stream', () => {
			arm('show-a', 6);
			Object.defineProperty(video, 'duration', {
				configurable: true,
				get: () => Number.POSITIVE_INFINITY
			});
			video.dispatchEvent(new Event('loadedmetadata'));
			video.currentTime = 1420;
			video.dispatchEvent(new Event('ended'));
			video.currentTime = 300;
			video.dispatchEvent(new Event('playing'));
			video.dispatchEvent(new Event('timeupdate'));
			expect(readPosition('show-a', 6, positions)).toBe(300);
			video.currentTime = 320;
			scope.flush();
			expect(readPosition('show-a', 6, positions)).toBe(320);
		});
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

	describe('an episode started but barely watched', () => {
		it('is marked started when its stream attaches, so a load that never opened still comes back to it', () => {
			arm('show-a', 6);
			expect(readPosition('show-a', 6, positions)).toBe(0);
		});

		it('attaching does not move a point already kept', () => {
			savePosition('show-a', 6, 600, 1420, positions);
			arm('show-a', 6);
			expect(readPosition('show-a', 6, positions)).toBe(600);
		});

		it('left in its first seconds, it stays started', () => {
			arm('show-a', 6);
			video.dispatchEvent(new Event('loadedmetadata'));
			playAt(5);
			scope.flush();
			expect(readPosition('show-a', 6, positions)).toBe(0);
		});

		it('a started mark opens from the start and is not judged finished, however short the stream', () => {
			savePosition('show-a', 6, 5, 60, positions);
			arm('show-a', 6);
			Object.defineProperty(video, 'duration', { configurable: true, get: () => 60 });
			video.currentTime = 0;
			video.dispatchEvent(new Event('loadedmetadata'));
			expect(video.currentTime).toBe(0);
			expect(readPosition('show-a', 6, positions)).toBe(0);
		});
	});

	it('a started mark does not wait for the length: the position is written as it plays', () => {
		// A mark at zero has nothing to judge, so a stream whose length is
		// not known keeps its position like any other.
		savePosition('show-a', 6, 0, Number.NaN, positions);
		arm('show-a', 6);
		Object.defineProperty(video, 'duration', {
			configurable: true,
			get: () => Number.POSITIVE_INFINITY
		});
		video.dispatchEvent(new Event('loadedmetadata'));
		video.currentTime = 600;
		video.dispatchEvent(new Event('pause'));
		expect(readPosition('show-a', 6, positions)).toBe(600);
	});
});

describe('a resume point holds the picture until the seek lands', () => {
	// The viewer must not see the stream's first frame, nor play it,
	// before the resume seek lands. The page shows its loading
	// treatment while the attach holds, and reveals at the point once
	// the seek landed there and playback started.
	let holds: boolean[];
	let play: ReturnType<typeof vi.fn>;
	let pause: ReturnType<typeof vi.fn>;
	let readyState: number;

	beforeEach(() => {
		holds = [];
		readyState = 1;
		Object.defineProperty(video, 'readyState', { configurable: true, get: () => readyState });
		play = vi.fn(() => {
			video.dispatchEvent(new Event('play'));
			video.dispatchEvent(new Event('playing'));
			return Promise.resolve();
		});
		pause = vi.fn();
		video.play = play as unknown as HTMLVideoElement['play'];
		video.pause = pause as unknown as HTMLVideoElement['pause'];
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	function armHolding(showId: string, episode: number) {
		// Built apart from the call, so the option the hold adds is not
		// an excess property of the input's literal.
		const input = {
			video,
			showId,
			episode,
			scope,
			positions,
			onResumeHold: (holding: boolean) => holds.push(holding)
		};
		armSourceScopedListeners(input);
	}

	const holding = () => holds.at(-1) === true;

	function metadataWith(duration: number) {
		Object.defineProperty(video, 'duration', { configurable: true, get: () => duration });
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
	}

	function seekedAt(seconds: number) {
		video.currentTime = seconds;
		video.dispatchEvent(new Event('seeked'));
	}

	const settle = () => new Promise<void>((r) => setTimeout(r, 0));

	it('holds from the attach, with autoplay off, when a kept point waits to be sought', () => {
		savePosition('show-a', 6, 600, 1420, positions);
		video.autoplay = true;
		armHolding('show-a', 6);
		expect(holding()).toBe(true);
		expect(video.autoplay).toBe(false);
	});

	it('reveals at the point only once the seek landed there and playback started', async () => {
		savePosition('show-a', 6, 600, 1420, positions);
		video.autoplay = true;
		armHolding('show-a', 6);
		metadataWith(1420);
		expect(video.currentTime).toBe(600);
		expect(holding()).toBe(true);
		expect(play).not.toHaveBeenCalled();
		seekedAt(600);
		expect(play).toHaveBeenCalledTimes(1);
		await settle();
		expect(holding()).toBe(false);
		expect(video.autoplay).toBe(true);
		// Our own start is not held back.
		expect(pause).not.toHaveBeenCalled();
	});

	it('a seek landing short of the point does not reveal; one slightly off it does', async () => {
		savePosition('show-a', 6, 600, 1420, positions);
		armHolding('show-a', 6);
		// The engine places the playhead at the stream's start before
		// the resume seeks; that is not the resume landing.
		seekedAt(0.2);
		metadataWith(1420);
		seekedAt(0.3);
		expect(play).not.toHaveBeenCalled();
		expect(holding()).toBe(true);
		// A seek snapped to a nearby keyframe has landed.
		seekedAt(599.2);
		expect(play).toHaveBeenCalledTimes(1);
		await settle();
		expect(holding()).toBe(false);
	});

	it('a seek landing before the stream buffered there reveals when frames play, not at the seek', async () => {
		savePosition('show-a', 6, 600, 1420, positions);
		armHolding('show-a', 6);
		// HLS reports the seek done before the fragment at the point is
		// in: playback has not started yet.
		play.mockImplementation(() => new Promise<void>(() => {}));
		metadataWith(1420);
		seekedAt(600);
		await settle();
		expect(holding()).toBe(true);
		video.dispatchEvent(new Event('playing'));
		expect(holding()).toBe(false);
	});

	it('autoplay refused reveals paused at the point once its frame is in', async () => {
		savePosition('show-a', 6, 600, 1420, positions);
		armHolding('show-a', 6);
		play.mockImplementation(() => Promise.reject(new DOMException('blocked', 'NotAllowedError')));
		metadataWith(1420);
		seekedAt(600);
		await settle();
		expect(holding()).toBe(true);
		readyState = 2;
		video.dispatchEvent(new Event('canplay'));
		expect(holding()).toBe(false);
		expect(video.currentTime).toBe(600);
	});

	it('autoplay refused with the frame already in reveals at once', async () => {
		savePosition('show-a', 6, 600, 1420, positions);
		armHolding('show-a', 6);
		play.mockImplementation(() => Promise.reject(new DOMException('blocked', 'NotAllowedError')));
		metadataWith(1420);
		readyState = 4;
		seekedAt(600);
		await settle();
		expect(holding()).toBe(false);
	});

	it('playback started by anything else during the hold is held back: the resume wins', () => {
		savePosition('show-a', 6, 600, 1420, positions);
		armHolding('show-a', 6);
		video.dispatchEvent(new Event('play'));
		expect(pause).toHaveBeenCalledTimes(1);
		metadataWith(1420);
		video.dispatchEvent(new Event('play'));
		expect(pause).toHaveBeenCalledTimes(2);
	});

	it('a length known late holds until it arrives, then reveals at the point', async () => {
		savePosition('show-a', 6, 600, Number.NaN, positions);
		armHolding('show-a', 6);
		metadataWith(Number.POSITIVE_INFINITY);
		expect(holding()).toBe(true);
		Object.defineProperty(video, 'duration', { configurable: true, get: () => 1420 });
		video.dispatchEvent(new Event('durationchange'));
		expect(video.currentTime).toBe(600);
		expect(holding()).toBe(true);
		seekedAt(600);
		await settle();
		expect(holding()).toBe(false);
	});

	it('a point in the last 90 seconds is forgotten and the stream revealed from its start', () => {
		savePosition('show-a', 6, 1400, Number.NaN, positions);
		armHolding('show-a', 6);
		metadataWith(1420);
		expect(holding()).toBe(false);
		expect(play).toHaveBeenCalledTimes(1);
		expect(video.currentTime).toBe(0);
	});

	it('a started mark, at zero, holds nothing', () => {
		savePosition('show-a', 6, 0, 1420, positions);
		video.autoplay = true;
		armHolding('show-a', 6);
		metadataWith(1420);
		expect(holds).not.toContain(true);
		expect(video.autoplay).toBe(true);
	});

	it('no point holds nothing', () => {
		video.autoplay = true;
		armHolding('show-a', 6);
		metadataWith(1420);
		expect(holds).not.toContain(true);
		expect(video.autoplay).toBe(true);
	});

	it("a pending recovery's point holds too, and reveals where it lands", async () => {
		recoveryResume.capture('show-a', 6, 432.5);
		armHolding('show-a', 6);
		expect(holding()).toBe(true);
		metadataWith(1420);
		expect(video.currentTime).toBe(432.5);
		seekedAt(432.5);
		await settle();
		expect(holding()).toBe(false);
	});

	it('the next attach ends the hold and gives autoplay back', () => {
		savePosition('show-a', 6, 600, 1420, positions);
		video.autoplay = true;
		armHolding('show-a', 6);
		scope.flush();
		expect(holding()).toBe(false);
		expect(video.autoplay).toBe(true);
		// The retired hold holds nothing back any more.
		video.dispatchEvent(new Event('play'));
		expect(pause).not.toHaveBeenCalled();
	});

	it('a length that never arrives reveals after fifteen seconds, and the point outlives it', () => {
		// The bound only stops hiding the picture. Where the episode
		// was left is not given up: nothing the stream plays writes over
		// it, and a length that does arrive still seeks there.
		vi.useFakeTimers();
		savePosition('show-a', 6, 600, Number.NaN, positions);
		armHolding('show-a', 6);
		metadataWith(Number.POSITIVE_INFINITY);
		vi.advanceTimersByTime(14_999);
		expect(holding()).toBe(true);
		vi.advanceTimersByTime(1);
		expect(holding()).toBe(false);
		expect(play).toHaveBeenCalledTimes(1);
		video.currentTime = 40;
		video.dispatchEvent(new Event('pause'));
		expect(readPosition('show-a', 6, positions)).toBe(600);
		Object.defineProperty(video, 'duration', { configurable: true, get: () => 1420 });
		video.dispatchEvent(new Event('durationchange'));
		expect(video.currentTime).toBe(600);
	});

	it('metadata that never arrives reveals after fifteen seconds, and leaving keeps the point', () => {
		vi.useFakeTimers();
		savePosition('show-a', 6, 600, 1420, positions);
		armHolding('show-a', 6);
		vi.advanceTimersByTime(15_000);
		expect(holding()).toBe(false);
		scope.flush();
		expect(readPosition('show-a', 6, positions)).toBe(600);
	});

	it('the bound starts over when the seek is issued', () => {
		// Opening and landing are each waits on one fragment: the first
		// brings the metadata, the one at the point lands the seek.
		vi.useFakeTimers();
		savePosition('show-a', 6, 600, 1420, positions);
		armHolding('show-a', 6);
		vi.advanceTimersByTime(10_000);
		metadataWith(1420);
		vi.advanceTimersByTime(14_999);
		expect(holding()).toBe(true);
		vi.advanceTimersByTime(1);
		expect(holding()).toBe(false);
		expect(play).toHaveBeenCalledTimes(1);
	});
});
