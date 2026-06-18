import { describe, expect, it, vi } from 'vitest';
import { createDebouncer } from './save-debounce';

describe('createDebouncer', () => {
	it('runs the scheduled task after the delay', () => {
		vi.useFakeTimers();
		const run = vi.fn();
		const d = createDebouncer(700);
		d.schedule('k', run);
		expect(run).not.toHaveBeenCalled();
		expect(d.pending('k')).toBe(true);
		vi.advanceTimersByTime(700);
		expect(run).toHaveBeenCalledTimes(1);
		expect(d.pending('k')).toBe(false);
		vi.useRealTimers();
	});

	it('coalesces a burst on the same key — only the latest run fires once', () => {
		vi.useFakeTimers();
		const first = vi.fn();
		const second = vi.fn();
		const third = vi.fn();
		const d = createDebouncer(700);
		d.schedule('k', first);
		vi.advanceTimersByTime(300);
		d.schedule('k', second);
		vi.advanceTimersByTime(300);
		d.schedule('k', third);
		vi.advanceTimersByTime(700);
		expect(first).not.toHaveBeenCalled();
		expect(second).not.toHaveBeenCalled();
		expect(third).toHaveBeenCalledTimes(1);
		vi.useRealTimers();
	});

	it('runs distinct keys independently', () => {
		vi.useFakeTimers();
		const a = vi.fn();
		const b = vi.fn();
		const d = createDebouncer(700);
		d.schedule('a', a);
		d.schedule('b', b);
		vi.advanceTimersByTime(700);
		expect(a).toHaveBeenCalledTimes(1);
		expect(b).toHaveBeenCalledTimes(1);
		vi.useRealTimers();
	});

	it('cancel drops a pending run', () => {
		vi.useFakeTimers();
		const run = vi.fn();
		const d = createDebouncer(700);
		d.schedule('k', run);
		d.cancel('k');
		expect(d.pending('k')).toBe(false);
		vi.advanceTimersByTime(700);
		expect(run).not.toHaveBeenCalled();
		vi.useRealTimers();
	});

	it('cancel on an unknown key is a no-op', () => {
		const d = createDebouncer(700);
		expect(() => d.cancel('nope')).not.toThrow();
		expect(d.pending('nope')).toBe(false);
	});
});
