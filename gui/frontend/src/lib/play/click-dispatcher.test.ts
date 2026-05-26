import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	createClickDispatcher,
	CLICK_DOUBLE_THRESHOLD_MS,
	CLICK_DOUBLE_MAX_DISTANCE_PX
} from './click-dispatcher';

const ORIGIN = { x: 0, y: 0 };

describe('createClickDispatcher', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('fires onSingle synchronously on the first click', () => {
		// `togglePlay` -> `video.play()` consumes transient user
		// activation, which a deferred setTimeout callback wouldn't
		// have. The single-click action must run inside the original
		// click gesture, not after a timer.
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double });

		d.click(ORIGIN);
		expect(single).toHaveBeenCalledTimes(1);
		expect(double).not.toHaveBeenCalled();
	});

	it('on a second click within the window, fires onSingleUndo then onDouble', () => {
		// First click already committed the single side-effect
		// (e.g. paused the video). The promotion to double must undo
		// it before applying the double-click action, so the net
		// effect of a double-click is purely `onDouble`.
		const single = vi.fn();
		const undo = vi.fn();
		const double = vi.fn();
		const order: string[] = [];
		const d = createClickDispatcher({
			onSingle: () => {
				single();
				order.push('single');
			},
			onSingleUndo: () => {
				undo();
				order.push('undo');
			},
			onDouble: () => {
				double();
				order.push('double');
			}
		});

		d.click(ORIGIN);
		vi.advanceTimersByTime(CLICK_DOUBLE_THRESHOLD_MS - 10);
		d.click(ORIGIN);

		expect(single).toHaveBeenCalledTimes(1);
		expect(undo).toHaveBeenCalledTimes(1);
		expect(double).toHaveBeenCalledTimes(1);
		expect(order).toEqual(['single', 'undo', 'double']);
	});

	it('skips onSingleUndo when the caller does not provide one', () => {
		// Not every caller wants undo (the single action might not be
		// reversible). The dispatcher still fires onDouble; the
		// single side-effect is left in place.
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double });

		d.click(ORIGIN);
		d.click(ORIGIN);

		expect(single).toHaveBeenCalledTimes(1);
		expect(double).toHaveBeenCalledTimes(1);
	});

	it('treats two clicks beyond the threshold as two synchronous singles', () => {
		const single = vi.fn();
		const undo = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({
			onSingle: single,
			onSingleUndo: undo,
			onDouble: double
		});

		d.click(ORIGIN);
		vi.advanceTimersByTime(CLICK_DOUBLE_THRESHOLD_MS);
		expect(single).toHaveBeenCalledTimes(1);

		d.click(ORIGIN);
		expect(single).toHaveBeenCalledTimes(2);
		expect(undo).not.toHaveBeenCalled();
		expect(double).not.toHaveBeenCalled();
	});

	it('three rapid clicks = one single, one undo+double, then a fresh single', () => {
		// Click 1 -> fires single synchronously, arms the window.
		// Click 2 (within window) -> undo + double, closes the window.
		// Click 3 -> fresh single (no longer in any window).
		const single = vi.fn();
		const undo = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({
			onSingle: single,
			onSingleUndo: undo,
			onDouble: double
		});

		d.click(ORIGIN);
		d.click(ORIGIN);
		expect(single).toHaveBeenCalledTimes(1);
		expect(undo).toHaveBeenCalledTimes(1);
		expect(double).toHaveBeenCalledTimes(1);

		d.click(ORIGIN);
		expect(single).toHaveBeenCalledTimes(2);
		expect(double).toHaveBeenCalledTimes(1); // unchanged
	});

	it('dispose clears the pending window so the next click starts fresh', () => {
		// Component teardown mid-window must not leave a stale "we
		// just fired a single" flag that promotes the next session's
		// first click to a double.
		const single = vi.fn();
		const undo = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({
			onSingle: single,
			onSingleUndo: undo,
			onDouble: double
		});

		d.click(ORIGIN);
		d.dispose();

		d.click(ORIGIN);
		expect(single).toHaveBeenCalledTimes(2);
		expect(undo).not.toHaveBeenCalled();
		expect(double).not.toHaveBeenCalled();
	});

	it('dispose is idempotent', () => {
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({ onSingle: single, onDouble: double });

		d.dispose();
		d.dispose();
		d.click(ORIGIN);
		expect(single).toHaveBeenCalledTimes(1);
	});

	it('respects a custom threshold for the upgrade window', () => {
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({
			onSingle: single,
			onDouble: double,
			thresholdMs: 500
		});

		d.click(ORIGIN);
		// 499 ms in, second click still upgrades to double.
		vi.advanceTimersByTime(499);
		d.click(ORIGIN);
		expect(double).toHaveBeenCalledTimes(1);

		// A second pair: first click then wait 500 ms before the
		// second click — window closed, both are singles.
		d.click(ORIGIN);
		vi.advanceTimersByTime(500);
		d.click(ORIGIN);
		expect(double).toHaveBeenCalledTimes(1); // unchanged
		expect(single).toHaveBeenCalledTimes(3);
	});

	it('does not promote to double when the second click lands far from the first', () => {
		// Standard double-click counting requires both clicks in the
		// same hit area. Without this, two quick clicks in different
		// parts of the video — e.g. play button, then a far corner —
		// would unexpectedly trigger fullscreen.
		const single = vi.fn();
		const undo = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({
			onSingle: single,
			onSingleUndo: undo,
			onDouble: double
		});

		d.click({ x: 100, y: 100 });
		// Second click within the time window but ~141px away — far
		// outside any reasonable double-click slop.
		d.click({ x: 200, y: 200 });

		expect(single).toHaveBeenCalledTimes(2);
		expect(undo).not.toHaveBeenCalled();
		expect(double).not.toHaveBeenCalled();
	});

	it('promotes to double when the second click is within the distance slop', () => {
		// Doubles tolerate a small pointer drift between presses —
		// users rarely click the exact same pixel twice. Anything
		// within CLICK_DOUBLE_MAX_DISTANCE_PX still counts as a double.
		const single = vi.fn();
		const undo = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({
			onSingle: single,
			onSingleUndo: undo,
			onDouble: double
		});

		d.click({ x: 100, y: 100 });
		// One pixel inside the slop circle (~99.99% of max distance).
		const drift = CLICK_DOUBLE_MAX_DISTANCE_PX - 1;
		d.click({ x: 100 + drift, y: 100 });

		expect(undo).toHaveBeenCalledTimes(1);
		expect(double).toHaveBeenCalledTimes(1);
	});

	it('respects a custom maxDistancePx for the hit-area check', () => {
		const single = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({
			onSingle: single,
			onDouble: double,
			maxDistancePx: 5
		});

		d.click({ x: 0, y: 0 });
		// 10px is outside the custom 5px slop.
		d.click({ x: 10, y: 0 });
		expect(single).toHaveBeenCalledTimes(2);
		expect(double).not.toHaveBeenCalled();
	});

	it('a far-away second click starts a fresh single-click window', () => {
		// After a too-distant second click, the dispatcher should
		// treat that second click as a brand-new first click — so a
		// rapid *third* click near the second one still promotes to
		// double.
		const single = vi.fn();
		const undo = vi.fn();
		const double = vi.fn();
		const d = createClickDispatcher({
			onSingle: single,
			onSingleUndo: undo,
			onDouble: double
		});

		d.click({ x: 0, y: 0 });
		d.click({ x: 500, y: 500 }); // far — fresh single
		d.click({ x: 500, y: 500 }); // close to the previous — double

		expect(single).toHaveBeenCalledTimes(2);
		expect(undo).toHaveBeenCalledTimes(1);
		expect(double).toHaveBeenCalledTimes(1);
	});
});
