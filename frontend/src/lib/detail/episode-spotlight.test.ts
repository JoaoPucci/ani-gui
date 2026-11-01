// @vitest-environment happy-dom
//
// The jumped-to episode's spotlight on a show's page and in the player.
import fc from 'fast-check';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	SPOTLIGHT_MS,
	browserSpotlightHost,
	episodeTileSelector,
	spotlightEpisode,
	type SpotlightHost
} from './episode-spotlight';
import { behaviorFor } from '$lib/motion/scroll';
import { reducedMotion } from '$lib/motion/reduced-motion.svelte';

/** A host whose frame and timer run only when the test says so. */
function fakeHost(tile: Element | null) {
	let nextId = 1;
	const frames = new Map<number, () => void>();
	const timers = new Map<number, { cb: () => void; ms: number }>();
	const scrolled: Element[] = [];
	const found: number[] = [];
	const host: SpotlightHost = {
		findTile: (n) => {
			found.push(n);
			return tile;
		},
		scrollTo: (el) => scrolled.push(el),
		nextFrame: (cb) => {
			const id = nextId++;
			frames.set(id, cb);
			return id;
		},
		cancelFrame: (id) => frames.delete(id),
		setTimer: (cb, ms) => {
			const id = nextId++;
			timers.set(id, { cb, ms });
			return id;
		},
		clearTimer: (id) => timers.delete(id)
	};
	const runFrame = () => {
		const all = [...frames.entries()];
		frames.clear();
		for (const [, cb] of all) cb();
	};
	const runTimer = () => {
		const all = [...timers.entries()];
		timers.clear();
		for (const [, t] of all) t.cb();
	};
	return { host, frames, timers, scrolled, found, runFrame, runTimer };
}

const tileEl = () => document.createElement('li');

describe('spotlightEpisode', () => {
	it('waits a frame, then scrolls to the tile and ends the spotlight after its time', () => {
		const tile = tileEl();
		const f = fakeHost(tile);
		const onEnd = vi.fn();
		spotlightEpisode(9, onEnd, f.host);

		expect(f.scrolled).toEqual([]);
		f.runFrame();
		expect(f.found).toEqual([9]);
		expect(f.scrolled).toEqual([tile]);
		expect([...f.timers.values()].map((t) => t.ms)).toEqual([SPOTLIGHT_MS]);
		expect(onEnd).not.toHaveBeenCalled();

		f.runTimer();
		expect(onEnd).toHaveBeenCalledTimes(1);
	});

	it('does nothing when the tile is not rendered yet, and starts no timer', () => {
		const f = fakeHost(null);
		const onEnd = vi.fn();
		spotlightEpisode(9, onEnd, f.host);
		f.runFrame();
		expect(f.scrolled).toEqual([]);
		expect(f.timers.size).toBe(0);
		expect(onEnd).not.toHaveBeenCalled();
	});

	it('cancels the pending frame when cleaned up before it runs', () => {
		const f = fakeHost(tileEl());
		const stop = spotlightEpisode(9, vi.fn(), f.host);
		stop();
		expect(f.frames.size).toBe(0);
		f.runFrame();
		expect(f.scrolled).toEqual([]);
	});

	it('clears the timer when cleaned up after the scroll, so the spotlight is not ended', () => {
		const f = fakeHost(tileEl());
		const onEnd = vi.fn();
		const stop = spotlightEpisode(9, onEnd, f.host);
		f.runFrame();
		stop();
		expect(f.timers.size).toBe(0);
		f.runTimer();
		expect(onEnd).not.toHaveBeenCalled();
	});

	it('holds its lifecycle for any order of frame, timer and cleanup', () => {
		const step = fc.constantFrom('frame', 'timer', 'cleanup');
		fc.assert(
			fc.property(
				fc.integer({ min: 1, max: 5000 }),
				fc.boolean(),
				fc.array(step, { maxLength: 8 }),
				(target, present, steps) => {
					const f = fakeHost(present ? tileEl() : null);
					const onEnd = vi.fn();
					const stop = spotlightEpisode(target, onEnd, f.host);
					let cleaned = false;
					let frameRan = false;
					let timerRan = false;
					for (const s of steps) {
						if (s === 'frame') {
							if (f.frames.size > 0) frameRan = true;
							f.runFrame();
						} else if (s === 'timer') {
							if (f.timers.size > 0) timerRan = true;
							f.runTimer();
						} else if (!cleaned) {
							stop();
							cleaned = true;
						}
					}
					// One look for the tile at most, and only for the target.
					expect(f.found.every((n) => n === target)).toBe(true);
					expect(f.found.length).toBe(frameRan ? 1 : 0);
					// A scroll only to a tile that is there, once.
					expect(f.scrolled.length).toBe(frameRan && present ? 1 : 0);
					// The spotlight ends only on its own timer.
					expect(onEnd).toHaveBeenCalledTimes(timerRan ? 1 : 0);
					if (timerRan) expect(present && frameRan).toBe(true);
					// Nothing is left scheduled once cleaned up.
					if (cleaned) {
						expect(f.frames.size).toBe(0);
						expect(f.timers.size).toBe(0);
					}
				}
			)
		);
	});
});

describe('episodeTileSelector', () => {
	it('selects the tile carrying the episode number', () => {
		fc.assert(
			fc.property(fc.integer({ min: 1, max: 100_000 }), (n) => {
				document.body.innerHTML = `<ul><li data-ep-num="${n}"></li><li data-ep-num="${n + 1}"></li></ul>`;
				const el = document.querySelector(episodeTileSelector(n));
				expect(el?.getAttribute('data-ep-num')).toBe(String(n));
			})
		);
		document.body.innerHTML = '';
	});
});

describe('browserSpotlightHost', () => {
	afterEach(() => {
		document.body.innerHTML = '';
		vi.restoreAllMocks();
	});

	it("finds a tile in the document and scrolls it to the centre the preference's way", () => {
		document.body.innerHTML = '<ul><li data-ep-num="4"></li></ul>';
		const host = browserSpotlightHost();
		const tile = host.findTile(4)!;
		expect(tile.getAttribute('data-ep-num')).toBe('4');
		expect(host.findTile(5)).toBeNull();

		const scrollIntoView = vi.fn();
		tile.scrollIntoView = scrollIntoView;
		host.scrollTo(tile);
		expect(scrollIntoView).toHaveBeenCalledWith({
			behavior: behaviorFor(reducedMotion.current),
			block: 'center'
		});
	});

	it('schedules and cancels through the window, called unbound as it is', () => {
		vi.useFakeTimers();
		try {
			const host = browserSpotlightHost();
			const ran = vi.fn();
			const id = host.setTimer(ran, 10);
			host.clearTimer(id);
			host.setTimer(ran, 10);
			vi.advanceTimersByTime(20);
			expect(ran).toHaveBeenCalledTimes(1);

			const frame = vi.fn();
			host.cancelFrame(host.nextFrame(frame));
			host.nextFrame(frame);
			vi.advanceTimersByTime(50);
			expect(frame).toHaveBeenCalledTimes(1);
		} finally {
			vi.useRealTimers();
		}
	});
});
