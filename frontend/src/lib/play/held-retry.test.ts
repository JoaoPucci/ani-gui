import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HELD_RETRY_TICK_MS, scheduleHeldRetry } from './held-retry';

describe('scheduleHeldRetry', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('fires when the delay is up while the runway stays long', () => {
		const fire = vi.fn();
		const cancel = scheduleHeldRetry({
			delayMs: 8000,
			runwaySeconds: () => 120,
			marginSeconds: 5,
			fire
		});
		vi.advanceTimersByTime(7999);
		expect(fire).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(fire).toHaveBeenCalledTimes(1);
		// Cancelling after the fact is nothing, and it fires once.
		cancel();
		vi.advanceTimersByTime(60_000);
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('fires early once the runway is down to the margin', () => {
		// A rate change or a seek after the hold was armed drains the
		// buffer sooner than the delay allowed for; the timer watches
		// the runway and asks for the engine while media is still to
		// spare.
		let runway = 120;
		const fire = vi.fn();
		scheduleHeldRetry({ delayMs: 16_000, runwaySeconds: () => runway, marginSeconds: 5, fire });
		vi.advanceTimersByTime(3000);
		expect(fire).not.toHaveBeenCalled();
		runway = 5;
		vi.advanceTimersByTime(HELD_RETRY_TICK_MS);
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('does not fire once cancelled', () => {
		const fire = vi.fn();
		const cancel = scheduleHeldRetry({
			delayMs: 2000,
			runwaySeconds: () => 120,
			marginSeconds: 5,
			fire
		});
		vi.advanceTimersByTime(1000);
		cancel();
		vi.advanceTimersByTime(60_000);
		expect(fire).not.toHaveBeenCalled();
	});

	it('fires at its delay when that is shorter than a tick, not a tick later', () => {
		const fire = vi.fn();
		scheduleHeldRetry({ delayMs: 100, runwaySeconds: () => 120, marginSeconds: 5, fire });
		vi.advanceTimersByTime(99);
		expect(fire).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(fire).toHaveBeenCalledTimes(1);
	});
});
