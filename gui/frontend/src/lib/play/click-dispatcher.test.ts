import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createClickDispatcher, CLICK_DOUBLE_THRESHOLD_MS } from './click-dispatcher';

describe('createClickDispatcher', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('fires onSingle after the double-click window elapses', () => {
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double });

		d.click();
		// Single must NOT fire synchronously — we have to wait the
		// whole window to know it isn't actually the first half of a
		// double.
		expect(single).not.toHaveBeenCalled();
		expect(double).not.toHaveBeenCalled();

		vi.advanceTimersByTime(CLICK_DOUBLE_THRESHOLD_MS - 1);
		expect(single).not.toHaveBeenCalled();

		vi.advanceTimersByTime(1);
		expect(single).toHaveBeenCalledTimes(1);
		expect(double).not.toHaveBeenCalled();
	});

	it('fires onDouble and cancels onSingle when a second click lands within the window', () => {
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double });

		d.click();
		vi.advanceTimersByTime(CLICK_DOUBLE_THRESHOLD_MS - 10);
		d.click();

		// onDouble fires immediately — no need to wait out the timer.
		expect(double).toHaveBeenCalledTimes(1);
		// The pending onSingle from the first click must be cancelled,
		// otherwise play/pause would still flip after fullscreen toggled.
		vi.advanceTimersByTime(1000);
		expect(single).not.toHaveBeenCalled();
	});

	it('treats two clicks beyond the threshold as two separate singles', () => {
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double });

		d.click();
		vi.advanceTimersByTime(CLICK_DOUBLE_THRESHOLD_MS);
		expect(single).toHaveBeenCalledTimes(1);

		d.click();
		vi.advanceTimersByTime(CLICK_DOUBLE_THRESHOLD_MS);
		expect(single).toHaveBeenCalledTimes(2);
		expect(double).not.toHaveBeenCalled();
	});

	it('three rapid clicks = one double + a fresh single', () => {
		// Click 1 → arms the timer.
		// Click 2 (within threshold) → fires onDouble, clears state.
		// Click 3 → starts a fresh single-click cycle.
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double });

		d.click();
		d.click();
		d.click();
		expect(double).toHaveBeenCalledTimes(1);
		expect(single).not.toHaveBeenCalled();

		vi.advanceTimersByTime(CLICK_DOUBLE_THRESHOLD_MS);
		expect(single).toHaveBeenCalledTimes(1);
	});

	it('dispose clears the pending single-click timer', () => {
		// Component teardown while a single-click timer is pending must
		// not fire onSingle into a destroyed component.
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double });

		d.click();
		d.dispose();
		vi.advanceTimersByTime(CLICK_DOUBLE_THRESHOLD_MS * 2);
		expect(single).not.toHaveBeenCalled();
		expect(double).not.toHaveBeenCalled();
	});

	it('dispose is idempotent and a fresh click after dispose still works', () => {
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double });

		d.dispose();
		d.dispose();

		d.click();
		vi.advanceTimersByTime(CLICK_DOUBLE_THRESHOLD_MS);
		expect(single).toHaveBeenCalledTimes(1);
	});

	it('respects a custom threshold', () => {
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double, thresholdMs: 500 });

		d.click();
		vi.advanceTimersByTime(499);
		expect(single).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(single).toHaveBeenCalledTimes(1);
	});
});
