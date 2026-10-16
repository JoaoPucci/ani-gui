import { afterEach, describe, expect, it, vi } from 'vitest';
import { QUALITY_SETTING_WAIT_MS, settledWithin, whenSettled } from './quality-sink';

afterEach(() => {
	vi.useRealTimers();
});

describe('settledWithin', () => {
	it('answers with the value when it comes in time', async () => {
		await expect(settledWithin(Promise.resolve('worst'), 100)).resolves.toBe('worst');
	});

	it('answers undefined once the bound passes, and for a failure', async () => {
		vi.useFakeTimers();
		const pending = settledWithin(new Promise<string>(() => {}), QUALITY_SETTING_WAIT_MS);
		vi.advanceTimersByTime(QUALITY_SETTING_WAIT_MS);
		await expect(pending).resolves.toBeUndefined();
		vi.useRealTimers();
		await expect(settledWithin(Promise.reject(new Error('down')), 100)).resolves.toBeUndefined();
	});

	it('bounds the wait to a few seconds', () => {
		expect(QUALITY_SETTING_WAIT_MS).toBeGreaterThanOrEqual(1_000);
		expect(QUALITY_SETTING_WAIT_MS).toBeLessThanOrEqual(5_000);
	});
});

describe('whenSettled', () => {
	it('hands over a value in hand at once', () => {
		const seen: unknown[] = [];
		whenSettled('720', (v) => seen.push(v));
		expect(seen).toEqual(['720']);
	});
});
