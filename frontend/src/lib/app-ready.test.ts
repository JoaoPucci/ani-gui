import { afterEach, describe, expect, it, vi } from 'vitest';

import { signalAppReady } from './app-ready';

describe('signalAppReady', () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it('tells the main process once the first page has been through two frames', () => {
		const frames: Array<() => void> = [];
		const raf = (cb: () => void) => frames.push(cb);
		const appReady = vi.fn();
		signalAppReady({ aniGui: { appReady } }, raf);
		expect(appReady).not.toHaveBeenCalled();
		frames.shift()?.();
		expect(appReady).not.toHaveBeenCalled();
		frames.shift()?.();
		expect(appReady).toHaveBeenCalledOnce();
	});

	it('says the page has mounted at once, before any frame, and only then waits for two frames', () => {
		// A window that is not on screen may get no frames at all (native
		// Wayland without GPU compositing): the main process maps it when
		// the mounted word is not followed by the rendered one
		// (electron/lib/app-window.cjs frameWake).
		const frames: Array<() => void> = [];
		const raf = (cb: () => void) => frames.push(cb);
		const calls: string[] = [];
		signalAppReady(
			{ aniGui: { appMounted: () => calls.push('mounted'), appReady: () => calls.push('ready') } },
			raf
		);
		expect(calls).toEqual(['mounted']);
		while (frames.length) frames.shift()?.();
		expect(calls).toEqual(['mounted', 'ready']);
	});

	it('does nothing outside the app (browser-only dev, tests)', () => {
		const frames: Array<() => void> = [];
		const raf = (cb: () => void) => frames.push(cb);
		expect(() => {
			signalAppReady(undefined, raf);
			signalAppReady({}, raf);
			while (frames.length) frames.shift()?.();
		}).not.toThrow();
	});

	it('uses the browser frame clock by default', () => {
		const frames: Array<() => void> = [];
		vi.stubGlobal('requestAnimationFrame', (cb: () => void) => frames.push(cb));
		const appReady = vi.fn();
		signalAppReady({ aniGui: { appReady } });
		while (frames.length) frames.shift()?.();
		expect(appReady).toHaveBeenCalledOnce();
	});
});
