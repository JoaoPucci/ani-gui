// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import { watchStart } from './start-watch';
import type { StartPhase } from './start-state';

function fakeVideo(state: { paused?: boolean; readyState?: number } = {}) {
	const v = document.createElement('video');
	let paused = state.paused ?? true;
	let readyState = state.readyState ?? 0;
	Object.defineProperty(v, 'paused', { get: () => paused, configurable: true });
	Object.defineProperty(v, 'readyState', { get: () => readyState, configurable: true });
	const pause = vi.fn(() => {
		paused = true;
	});
	Object.defineProperty(v, 'pause', { value: pause, configurable: true });
	return {
		v,
		pause,
		set(next: { paused?: boolean; readyState?: number }) {
			paused = next.paused ?? paused;
			readyState = next.readyState ?? readyState;
		},
		fire(name: string) {
			v.dispatchEvent(new Event(name));
		}
	};
}

function arm(video: HTMLVideoElement, initial?: StartPhase) {
	const seen: StartPhase[] = [];
	const w = watchStart(video, (p) => seen.push(p), initial);
	return { w, seen };
}

describe('watchStart', () => {
	it('an attach to an autoplaying element starts, and the first playing ends it', () => {
		const f = fakeVideo();
		const { w, seen } = arm(f.v);
		w.attach(true);
		expect(w.phase()).toBe('starting');
		f.fire('playing');
		expect(w.phase()).toBe('playing');
		expect(seen).toEqual(['starting', 'playing']);
	});

	it('no event before the first fragment moves a start, however long it takes', () => {
		const f = fakeVideo();
		const { w } = arm(f.v);
		w.attach(true);
		f.fire('loadedmetadata');
		f.fire('progress');
		f.fire('waiting');
		expect(w.phase()).toBe('starting');
	});

	it('a waiting while playing is a stall, and a seek inside the buffer is over at seeked', () => {
		const f = fakeVideo({ paused: false, readyState: 4 });
		const { w } = arm(f.v, 'playing');
		f.fire('waiting');
		expect(w.phase()).toBe('stalled');
		f.fire('playing');
		f.fire('seeking');
		expect(w.phase()).toBe('stalled');
		f.fire('seeked');
		expect(w.phase()).toBe('playing');
	});

	it('cancel stops a start: the element is paused, which fires nothing, and the phase says paused', () => {
		const f = fakeVideo();
		const { w, seen } = arm(f.v);
		w.attach(true);
		w.cancel();
		expect(f.pause).toHaveBeenCalledTimes(1);
		expect(w.phase()).toBe('paused');
		expect(seen.at(-1)).toBe('paused');
	});

	it('autoplay refused: the data in and still paused reads paused', () => {
		const f = fakeVideo();
		const { w } = arm(f.v);
		w.attach(true);
		f.set({ readyState: 4 });
		f.fire('canplaythrough');
		expect(w.phase()).toBe('paused');
	});

	it('reports only changes', () => {
		const f = fakeVideo();
		const { w, seen } = arm(f.v);
		w.attach(true);
		w.attach(true);
		f.fire('play');
		expect(seen).toEqual(['starting']);
	});

	it('dispose stops listening', () => {
		const f = fakeVideo();
		const { w, seen } = arm(f.v);
		w.attach(true);
		w.dispose();
		f.fire('playing');
		expect(seen).toEqual(['starting']);
	});
});
