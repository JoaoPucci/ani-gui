import { describe, expect, it, vi } from 'vitest';
import { momentOf, watchMoment, type MomentDocument, type VideoLike } from './moment';

function fakeDoc(videos: VideoLike[], fullscreen: unknown = null) {
	const listeners = new Map<string, Set<() => void>>();
	let full = fullscreen;
	const doc: MomentDocument & {
		fire(type: string): void;
		count(): number;
		setFull(v: unknown): void;
	} = {
		get fullscreenElement() {
			return full;
		},
		setFull(v) {
			full = v;
		},
		querySelectorAll: () => videos,
		addEventListener(type, l) {
			if (!listeners.has(type)) listeners.set(type, new Set());
			listeners.get(type)?.add(l);
		},
		removeEventListener(type, l) {
			listeners.get(type)?.delete(l);
		},
		fire(type) {
			for (const l of listeners.get(type) ?? []) l();
		},
		count() {
			return [...listeners.values()].reduce((a, s) => a + s.size, 0);
		}
	};
	return doc;
}

describe('what the window is doing', () => {
	it('reads playing from any video that is neither paused nor ended', () => {
		expect(momentOf(fakeDoc([])).playing).toBe(false);
		expect(momentOf(fakeDoc([{ paused: true, ended: false }])).playing).toBe(false);
		expect(momentOf(fakeDoc([{ paused: false, ended: true }])).playing).toBe(false);
		expect(
			momentOf(
				fakeDoc([
					{ paused: true, ended: false },
					{ paused: false, ended: false }
				])
			).playing
		).toBe(true);
	});

	it('reads fullscreen from the fullscreen element', () => {
		expect(momentOf(fakeDoc([], null)).fullscreen).toBe(false);
		expect(momentOf(fakeDoc([], {})).fullscreen).toBe(true);
	});

	it('reports at once, on media and fullscreen events, and stops when asked', () => {
		const video = { paused: true, ended: false };
		const doc = fakeDoc([video]);
		const seen = vi.fn();
		const stop = watchMoment(doc, seen);
		expect(seen).toHaveBeenLastCalledWith({ playing: false, fullscreen: false });
		video.paused = false;
		doc.fire('play');
		expect(seen).toHaveBeenLastCalledWith({ playing: true, fullscreen: false });
		doc.setFull({});
		doc.fire('fullscreenchange');
		expect(seen).toHaveBeenLastCalledWith({ playing: true, fullscreen: true });
		for (const e of ['playing', 'pause', 'ended', 'emptied']) doc.fire(e);
		expect(seen).toHaveBeenCalledTimes(7);
		stop();
		expect(doc.count()).toBe(0);
	});
});
