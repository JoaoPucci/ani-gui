import { describe, expect, test, vi } from 'vitest';
import { createAnimationGate, shiftedSurvivorIds } from './animation-gate';

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
	test('starts with both buckets empty (the dedupe-on-load case)', () => {
		const c = clock();
		const gate = createAnimationGate(350, c);
		expect(gate.shouldAnimateRemoval('aa-1')).toBe(false);
		expect(gate.shouldAnimateShift('aa-2')).toBe(false);
	});

	test('open() puts ids in their own bucket until holdMs elapses', () => {
		const c = clock();
		const gate = createAnimationGate(350, c);

		gate.open(['del-1', 'del-2'], ['surv-3', 'surv-4']);
		expect(gate.shouldAnimateRemoval('del-1')).toBe(true);
		expect(gate.shouldAnimateRemoval('del-2')).toBe(true);
		expect(gate.shouldAnimateShift('surv-3')).toBe(true);
		// Removed ids must NOT animate flip, and shifted ids must
		// NOT animate the scale-out — that crosstalk is exactly the
		// Codex P2 #3369281412 leak this split exists to prevent.
		expect(gate.shouldAnimateShift('del-1')).toBe(false);
		expect(gate.shouldAnimateRemoval('surv-3')).toBe(false);

		c.advance(349);
		expect(gate.shouldAnimateRemoval('del-1')).toBe(true);
		// surv-4 is still available (never read) until the window
		// expires; surv-3 was consumed above so checking it would
		// return false even without the timer (covered separately).
		expect(gate.shouldAnimateShift('surv-4')).toBe(true);
		c.advance(1);
		expect(gate.shouldAnimateRemoval('del-1')).toBe(false);
		expect(gate.shouldAnimateShift('surv-4')).toBe(false);
	});

	test('shouldAnimateShift is ONE-SHOT — a second call for the same id returns false', () => {
		// Scenario Codex P2 #3369293607 caught: user deletes A.
		// During the 350ms window a background dedupe mutation
		// removes a different row, causing remaining survivors to
		// re-flip. The shift gate must not re-fire on survivors
		// it already animated for the user's delete.
		const c = clock();
		const gate = createAnimationGate(350, c);
		gate.open([], ['B']);
		expect(gate.shouldAnimateShift('B')).toBe(true);
		expect(gate.shouldAnimateShift('B')).toBe(false);
	});

	test('shouldAnimateRemoval is NOT consumed (deleted nodes can only fire once anyway)', () => {
		// The removed element leaves the DOM after its out:scale
		// completes, so an idempotent (non-consuming) read is fine
		// — and keeps the gate simpler to reason about.
		const c = clock();
		const gate = createAnimationGate(350, c);
		gate.open(['A'], []);
		expect(gate.shouldAnimateRemoval('A')).toBe(true);
		expect(gate.shouldAnimateRemoval('A')).toBe(true);
	});

	test('a concurrent dedupe-removal of a shifted survivor does NOT animate as a removal', () => {
		// Scenario the Codex P2 caught:
		//   1. User deletes A. shifted = [B, C].
		//   2. During the 350ms window, an unrelated
		//      loadContinueWatchingState callback resolves B's
		//      Kitsu match and dedupe removes B from the each.
		//   3. The factory MUST see shouldAnimateRemoval('B') ===
		//      false — that scale-out is from dedupe, not from the
		//      user delete.
		const c = clock();
		const gate = createAnimationGate(350, c);
		gate.open(['A'], ['B', 'C']);
		expect(gate.shouldAnimateRemoval('B')).toBe(false);
	});

	test('open() called again replaces both sets and resets the timer', () => {
		const c = clock();
		const gate = createAnimationGate(350, c);

		gate.open(['A'], ['B']);
		c.advance(300);
		expect(gate.shouldAnimateRemoval('A')).toBe(true);

		gate.open(['X'], ['Y']);
		expect(gate.shouldAnimateRemoval('A')).toBe(false);
		expect(gate.shouldAnimateShift('B')).toBe(false);
		expect(gate.shouldAnimateRemoval('X')).toBe(true);
		expect(gate.shouldAnimateShift('Y')).toBe(true);
		expect(c.pending()).toBe(1); // first timer cancelled

		c.advance(349);
		expect(gate.shouldAnimateRemoval('X')).toBe(true);
		c.advance(1);
		expect(gate.shouldAnimateRemoval('X')).toBe(false);
	});

	test('default factory delegates open + auto-close to global setTimeout', () => {
		vi.useFakeTimers();
		try {
			const gate = createAnimationGate(50);
			gate.open(['x'], []);
			expect(gate.shouldAnimateRemoval('x')).toBe(true);
			vi.advanceTimersByTime(50);
			expect(gate.shouldAnimateRemoval('x')).toBe(false);
		} finally {
			vi.useRealTimers();
		}
	});

	test('default factory routes a second open through global clearTimeout', () => {
		// Coverage hook for `defaultDeps.clearTimeout` — the lambda
		// only runs when an active timer needs cancelling, so a
		// single-open test misses it. Calling open() twice in a row
		// exercises the cancel path.
		vi.useFakeTimers();
		try {
			const gate = createAnimationGate(50);
			gate.open(['x'], []);
			gate.open(['y'], []); // cancels the first timer
			expect(gate.shouldAnimateRemoval('x')).toBe(false);
			expect(gate.shouldAnimateRemoval('y')).toBe(true);
		} finally {
			vi.useRealTimers();
		}
	});
});

describe('shiftedSurvivorIds', () => {
	test('returns every survivor positioned after the first removed', () => {
		// Snapshot: [A, B, C, D, E]. Remove C and D. Only E sits
		// after the first removed index (2) and survives, so E is
		// the only shifted id.
		expect(shiftedSurvivorIds(['A', 'B', 'C', 'D', 'E'], ['C', 'D'])).toEqual(['E']);
	});

	test('returns nothing for a trailing delete (no survivor shifts)', () => {
		expect(shiftedSurvivorIds(['A', 'B', 'C'], ['B', 'C'])).toEqual([]);
	});

	test('skips removed ids when scanning past the first-removed index', () => {
		// Removed = [B, D]. Survivors after first removed (B at 1):
		// [C, D, E]. Filter out the removed → [C, E].
		expect(shiftedSurvivorIds(['A', 'B', 'C', 'D', 'E'], ['B', 'D'])).toEqual(['C', 'E']);
	});

	test('returns nothing when nothing is removed', () => {
		expect(shiftedSurvivorIds(['A', 'B'], [])).toEqual([]);
	});

	test('returns nothing when the removed ids are not in the snapshot', () => {
		// Defensive: if the snapshot doesn't include any of the
		// removed ids (already evicted before the snapshot was
		// captured), there's no position to shift survivors from.
		expect(shiftedSurvivorIds(['A', 'B'], ['X', 'Y'])).toEqual([]);
	});
});
