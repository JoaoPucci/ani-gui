import { describe, expect, test, vi } from 'vitest';
import { createAnimationGate, idsAffectedByDelete } from './animation-gate';

/**
 * Tiny shim so each test can step time deterministically without
 * vi.useFakeTimers globally — Svelte's test environment is fussy
 * about which timer the framework owns. The shim records pending
 * callbacks; `advance(ms)` triggers any whose target time is ≤ ms.
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
	test('starts empty (the load-time dedupe-flicker case)', () => {
		const c = clock();
		const gate = createAnimationGate(350, c);
		expect(gate.shouldAnimate('aa-1')).toBe(false);
	});

	test('open(ids) flips on only the listed ids until holdMs elapses', () => {
		const c = clock();
		const gate = createAnimationGate(350, c);

		gate.open(['aa-1', 'aa-2']);
		expect(gate.shouldAnimate('aa-1')).toBe(true);
		expect(gate.shouldAnimate('aa-2')).toBe(true);
		// Unrelated row that happened to dedupe-collapse during the
		// window must NOT animate — Codex P2 #3369269241.
		expect(gate.shouldAnimate('aa-9')).toBe(false);

		c.advance(349);
		expect(gate.shouldAnimate('aa-1')).toBe(true);

		c.advance(1);
		expect(gate.shouldAnimate('aa-1')).toBe(false);
	});

	test('open() called again replaces the set and resets the timer', () => {
		const c = clock();
		const gate = createAnimationGate(350, c);

		gate.open(['aa-1']);
		c.advance(300);
		expect(gate.shouldAnimate('aa-1')).toBe(true);

		// User triggers another delete on a different row while
		// the first transition is still in flight. The second
		// open() replaces the set + restarts the timer.
		gate.open(['aa-2']);
		expect(gate.shouldAnimate('aa-1')).toBe(false);
		expect(gate.shouldAnimate('aa-2')).toBe(true);
		expect(c.pending()).toBe(1); // first timer cancelled

		c.advance(349);
		expect(gate.shouldAnimate('aa-2')).toBe(true);
		c.advance(1);
		expect(gate.shouldAnimate('aa-2')).toBe(false);
	});

	test('the default delegates to global setTimeout/clearTimeout', () => {
		vi.useFakeTimers();
		try {
			const gate = createAnimationGate(50);
			gate.open(['x']);
			expect(gate.shouldAnimate('x')).toBe(true);
			vi.advanceTimersByTime(50);
			expect(gate.shouldAnimate('x')).toBe(false);
		} finally {
			vi.useRealTimers();
		}
	});
});

describe('idsAffectedByDelete', () => {
	test('returns the removed ids + every survivor positioned after the first removed', () => {
		// Snapshot: [A, B, C, D, E]. Remove C and D. Survivors that
		// shift left to close the gap: E (the only one after the
		// first removed index 2). A and B don't shift.
		const result = idsAffectedByDelete(['A', 'B', 'C', 'D', 'E'], ['C', 'D']);
		expect(result.sort()).toEqual(['C', 'D', 'E']);
	});

	test('returns just the removed ids when nothing follows them', () => {
		// Trailing delete: nothing shifts because there are no
		// survivors past the removed range.
		const result = idsAffectedByDelete(['A', 'B', 'C'], ['B', 'C']);
		expect(result.sort()).toEqual(['B', 'C']);
	});

	test('skips removed ids when computing the shifted survivors', () => {
		// Removed = [B, D]. Survivors after first removed (B at
		// index 1): C, D, E. Filter out the removed ids: C, E.
		const result = idsAffectedByDelete(['A', 'B', 'C', 'D', 'E'], ['B', 'D']);
		expect(result.sort()).toEqual(['B', 'C', 'D', 'E']);
	});

	test('returns an empty list when nothing is removed', () => {
		expect(idsAffectedByDelete(['A', 'B'], [])).toEqual([]);
	});

	test('returns the removed ids when none of them appear in the snapshot', () => {
		// Defensive: if the snapshot doesn't include the removed
		// ids (e.g., already evicted before the snapshot was taken),
		// there's no position to shift survivors from. Still report
		// the removed ids so out:scale fires on whatever DOM nodes
		// happen to be tagged with them.
		expect(idsAffectedByDelete(['A', 'B'], ['X', 'Y']).sort()).toEqual(['X', 'Y']);
	});
});
