import { describe, expect, test, vi } from 'vitest';
import { createAnimationGate } from './animation-gate';

/**
 * Tiny shim so each test can step time deterministically without
 * vi.useFakeTimers globally — Svelte's test environment is fussy
 * about which timer the framework owns. The shim records pending
 * callbacks; `flush(ms)` triggers any whose target time is ≤ ms.
 */
function clock() {
	let now = 0;
	type Pending = { id: number; at: number; cb: () => void };
	const pending: Pending[] = [];
	let nextId = 1;
	return {
		setTimeout(cb: () => void, ms: number) {
			const id = nextId++;
			pending.push({ id, at: now + ms, cb });
			return id;
		},
		clearTimeout(handle: unknown) {
			const idx = pending.findIndex((p) => p.id === handle);
			if (idx >= 0) pending.splice(idx, 1);
		},
		advance(ms: number) {
			now += ms;
			const due = pending.filter((p) => p.at <= now);
			for (const p of due) {
				const idx = pending.indexOf(p);
				if (idx >= 0) pending.splice(idx, 1);
				p.cb();
			}
		},
		pending() {
			return pending.length;
		}
	};
}

describe('createAnimationGate', () => {
	test('starts closed (the load-time dedupe-flicker case)', () => {
		const c = clock();
		const gate = createAnimationGate(350, c);
		expect(gate.isOn()).toBe(false);
	});

	test('open() flips on until holdMs elapses (user-confirmed delete case)', () => {
		const c = clock();
		const gate = createAnimationGate(350, c);

		gate.open();
		expect(gate.isOn()).toBe(true);

		c.advance(349);
		expect(gate.isOn()).toBe(true);

		c.advance(1);
		expect(gate.isOn()).toBe(false);
	});

	test('open() called again resets the timer instead of stacking', () => {
		const c = clock();
		const gate = createAnimationGate(350, c);

		gate.open();
		c.advance(300);
		expect(gate.isOn()).toBe(true);

		// User triggers another delete while the first transition is
		// still in flight. The second open() must extend the window,
		// not let the first timer close the gate at t=350.
		gate.open();
		c.advance(100); // total 400ms from first open, 100ms from second
		expect(gate.isOn()).toBe(true);
		expect(c.pending()).toBe(1); // and the first timer was cancelled

		c.advance(250);
		expect(gate.isOn()).toBe(false);
	});

	test('the default delegates to global setTimeout/clearTimeout', () => {
		// Cheap sanity check that the no-deps factory still works —
		// production uses the real timers, tests inject the shim.
		vi.useFakeTimers();
		try {
			const gate = createAnimationGate(50);
			gate.open();
			expect(gate.isOn()).toBe(true);
			vi.advanceTimersByTime(50);
			expect(gate.isOn()).toBe(false);
		} finally {
			vi.useRealTimers();
		}
	});
});
