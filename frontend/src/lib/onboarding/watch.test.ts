// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SETTLE_MS } from './gate';
import { PlaceWatcher, browserWatchDeps, type WatchDeps } from './watch';

/** An IntersectionObserver the test drives by hand. */
class FakeIO {
	observed = new Set<Element>();
	constructor(readonly cb: IntersectionObserverCallback) {}
	observe(el: Element) {
		this.observed.add(el);
	}
	unobserve(el: Element) {
		this.observed.delete(el);
	}
	disconnect() {
		this.observed.clear();
	}
	fire(el: Element, top: number, bottom: number, width = 100) {
		const entry = {
			target: el,
			boundingClientRect: { top, bottom, width, height: bottom - top }
		} as unknown as IntersectionObserverEntry;
		this.cb([entry], this as unknown as IntersectionObserver);
	}
}

let io: FakeIO;
let settled: string[][];
let watcher: PlaceWatcher | null;
const flush = () => new Promise((r) => setTimeout(r, 0));

function deps(): WatchDeps {
	return {
		doc: document,
		viewTop: () => 64,
		viewHeight: () => 800,
		observe: (cb) => {
			io = new FakeIO(cb);
			return io as unknown as IntersectionObserver;
		},
		setTimer: (fn, ms) => setTimeout(fn, ms),
		clearTimer: (t) => clearTimeout(t)
	};
}

function start(places: string[]) {
	watcher = new PlaceWatcher(places, (s) => settled.push([...s]), deps());
}

beforeEach(() => {
	document.body.innerHTML = '<main><div data-onboard="strip" id="strip"></div></main>';
	settled = [];
	watcher = null;
	vi.useFakeTimers();
});

afterEach(() => {
	watcher?.stop();
	vi.useRealTimers();
	document.body.innerHTML = '';
});

describe('PlaceWatcher', () => {
	it('settles a place that stays on screen for SETTLE_MS', () => {
		start(['strip']);
		const el = document.getElementById('strip')!;
		expect(io.observed.has(el)).toBe(true);
		io.fire(el, 200, 400);
		vi.advanceTimersByTime(SETTLE_MS - 1);
		expect(settled).toEqual([]);
		vi.advanceTimersByTime(1);
		expect(settled).toEqual([['strip']]);
	});

	it('starts again when the place moves before it settles', () => {
		start(['strip']);
		const el = document.getElementById('strip')!;
		io.fire(el, 200, 400);
		vi.advanceTimersByTime(SETTLE_MS - 100);
		io.fire(el, 180, 380);
		vi.advanceTimersByTime(SETTLE_MS - 1);
		expect(settled).toEqual([]);
		vi.advanceTimersByTime(1);
		expect(settled).toEqual([['strip']]);
	});

	it('never settles a place off screen, and unsettles one that leaves', () => {
		start(['strip']);
		const el = document.getElementById('strip')!;
		io.fire(el, 900, 1100);
		vi.advanceTimersByTime(SETTLE_MS * 3);
		expect(settled).toEqual([]);
		io.fire(el, 200, 400);
		vi.advanceTimersByTime(SETTLE_MS);
		io.fire(el, -400, -200);
		expect(settled).toEqual([['strip'], []]);
	});

	it('finds a place that mounts late', async () => {
		document.body.innerHTML = '<main></main>';
		start(['strip']);
		expect(io.observed.size).toBe(0);
		const late = document.createElement('div');
		late.dataset.onboard = 'strip';
		document.querySelector('main')!.appendChild(late);
		vi.useRealTimers();
		await flush();
		expect(io.observed.has(late)).toBe(true);
	});

	it('lets go of a place that leaves the page', async () => {
		start(['strip']);
		const el = document.getElementById('strip')!;
		io.fire(el, 200, 400);
		vi.advanceTimersByTime(SETTLE_MS);
		el.remove();
		vi.useRealTimers();
		await flush();
		expect(io.observed.has(el)).toBe(false);
		expect(settled.at(-1)).toEqual([]);
	});

	it('watches nothing once stopped', () => {
		start(['strip']);
		const el = document.getElementById('strip')!;
		io.fire(el, 200, 400);
		watcher!.stop();
		vi.advanceTimersByTime(SETTLE_MS);
		expect(settled).toEqual([]);
		expect(io.observed.size).toBe(0);
	});

	it('a scroll while the place stays in full view starts its settling again', () => {
		start(['strip']);
		const el = document.getElementById('strip')!;
		el.getBoundingClientRect = () => ({ top: 200, bottom: 400, width: 100 }) as DOMRect;
		io.fire(el, 200, 400);
		vi.advanceTimersByTime(SETTLE_MS - 100);
		// No threshold crosses: only the scroll says the page moved.
		document.dispatchEvent(new Event('scroll'));
		vi.advanceTimersByTime(SETTLE_MS - 1);
		expect(settled).toEqual([]);
		vi.advanceTimersByTime(1);
		expect(settled).toEqual([['strip']]);
	});

	it('a scroll that takes the place out of view unsettles it', () => {
		start(['strip']);
		const el = document.getElementById('strip')!;
		io.fire(el, 200, 400);
		vi.advanceTimersByTime(SETTLE_MS);
		el.getBoundingClientRect = () => ({ top: -600, bottom: -400, width: 100 }) as DOMRect;
		document.dispatchEvent(new Event('scroll'));
		expect(settled).toEqual([['strip'], []]);
	});
});

describe('browserWatchDeps', () => {
	it('reads the top bar and the window, and uses the page’s observer and clock', () => {
		document.body.innerHTML = '<header class="topbar"></header>';
		document.querySelector('.topbar')!.getBoundingClientRect = () => ({ bottom: 64 }) as DOMRect;
		const made: IntersectionObserverInit[] = [];
		vi.stubGlobal(
			'IntersectionObserver',
			class {
				constructor(_cb: IntersectionObserverCallback, init: IntersectionObserverInit) {
					made.push(init);
				}
			}
		);
		const deps = browserWatchDeps(document);
		expect(deps.viewTop()).toBe(64);
		expect(deps.viewHeight()).toBe(window.innerHeight);
		deps.observe(() => {}, { threshold: [0] });
		expect(made).toEqual([{ threshold: [0] }]);
		const fn = vi.fn();
		const t = deps.setTimer(fn, 10);
		deps.clearTimer(t);
		vi.advanceTimersByTime(20);
		expect(fn).not.toHaveBeenCalled();
		vi.unstubAllGlobals();
	});

	it('reads no top bar as the window’s top', () => {
		document.body.innerHTML = '';
		expect(browserWatchDeps(document).viewTop()).toBe(0);
	});
});
