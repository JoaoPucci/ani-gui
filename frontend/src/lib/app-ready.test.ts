import { describe, expect, it, vi } from 'vitest';

import { signalAppReady } from './app-ready';

describe('signalAppReady', () => {
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

	it('does nothing outside the app (browser-only dev, tests)', () => {
		const frames: Array<() => void> = [];
		const raf = (cb: () => void) => frames.push(cb);
		expect(() => {
			signalAppReady(undefined, raf);
			signalAppReady({}, raf);
			while (frames.length) frames.shift()?.();
		}).not.toThrow();
	});
});
