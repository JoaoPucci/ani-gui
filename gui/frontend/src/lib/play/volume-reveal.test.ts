import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createVolumeReveal, VOLUME_REVEAL_HOLD_MS } from './volume-reveal';

describe('createVolumeReveal', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it('flips revealed=true synchronously on trigger, false after the hold window', () => {
		const events: boolean[] = [];
		const reveal = createVolumeReveal((visible) => events.push(visible));

		reveal.trigger();
		// First push is the synchronous reveal — must land before any timer
		// fires so the volume pill expands on the same frame as the keypress.
		expect(events).toEqual([true]);

		vi.advanceTimersByTime(VOLUME_REVEAL_HOLD_MS - 1);
		expect(events).toEqual([true]);

		vi.advanceTimersByTime(1);
		expect(events).toEqual([true, false]);
	});

	it('rapid retriggers reset the hide timer (auto-repeat does not blink)', () => {
		// Holding ArrowUp fires `keydown` ~30 ms apart; if every trigger
		// scheduled an independent timer the pill would flash off the
		// first time the hold-window elapsed even while the key was
		// still held. Each trigger must replace any pending hide.
		const events: boolean[] = [];
		const reveal = createVolumeReveal((visible) => events.push(visible));

		reveal.trigger();
		vi.advanceTimersByTime(VOLUME_REVEAL_HOLD_MS - 100);
		reveal.trigger(); // refresh — does not re-emit `true` (already true)
		vi.advanceTimersByTime(VOLUME_REVEAL_HOLD_MS - 100);
		// We have advanced 2 * (HOLD - 100) ms in total — well past one
		// hold window. Pill must still be visible because the second
		// trigger restarted the timer.
		expect(events).toEqual([true]);

		vi.advanceTimersByTime(200); // now past the second trigger's hold window
		expect(events).toEqual([true, false]);
	});

	it('subsequent trigger after natural hide emits a fresh true', () => {
		const events: boolean[] = [];
		const reveal = createVolumeReveal((visible) => events.push(visible));

		reveal.trigger();
		vi.advanceTimersByTime(VOLUME_REVEAL_HOLD_MS);
		expect(events).toEqual([true, false]);

		reveal.trigger();
		expect(events).toEqual([true, false, true]);
	});

	it('dispose clears the pending timer and forces hidden', () => {
		// Component teardown (route change while a hide is pending) must
		// not leave a setTimeout that later writes into a destroyed
		// component's state.
		const events: boolean[] = [];
		const reveal = createVolumeReveal((visible) => events.push(visible));

		reveal.trigger();
		reveal.dispose();
		expect(events).toEqual([true, false]);

		vi.advanceTimersByTime(VOLUME_REVEAL_HOLD_MS * 2);
		// No further events — the timer never fired.
		expect(events).toEqual([true, false]);
	});

	it('dispose is idempotent (no callback when nothing pending)', () => {
		const events: boolean[] = [];
		const reveal = createVolumeReveal((visible) => events.push(visible));

		reveal.dispose();
		// No state to clear — must not synthesise a false event.
		expect(events).toEqual([]);

		reveal.dispose();
		expect(events).toEqual([]);
	});

	it('trigger after dispose still works (re-entrant lifecycle)', () => {
		// Defensive: the page-level `$effect` could in theory re-register
		// the reveal after teardown if a parent reuses the instance.
		// Trigger after dispose must behave like a fresh instance.
		const events: boolean[] = [];
		const reveal = createVolumeReveal((visible) => events.push(visible));

		reveal.dispose();
		reveal.trigger();
		expect(events).toEqual([true]);

		vi.advanceTimersByTime(VOLUME_REVEAL_HOLD_MS);
		expect(events).toEqual([true, false]);
	});
});
