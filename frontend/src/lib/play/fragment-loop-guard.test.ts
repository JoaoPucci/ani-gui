import { describe, expect, it } from 'vitest';
import {
	FRAGMENT_LOAD_ALLOWANCE,
	FRAGMENT_LOAD_WINDOW_MS,
	FragmentLoopGuard
} from './fragment-loop-guard';

function clock(start = 0) {
	let now = start;
	return {
		now: () => now,
		advance(ms: number) {
			now += ms;
		}
	};
}

describe('the fragment loop guard', () => {
	it('lets a playing stream load every fragment once', () => {
		const t = clock();
		const guard = new FragmentLoopGuard(t.now);
		for (let sn = 1; sn <= 300; sn++) {
			expect(guard.loaded(`0:${sn}`)).toBe(false);
			t.advance(50);
		}
	});

	it('trips when one fragment loads more often than the allowance inside the window', () => {
		const t = clock();
		const guard = new FragmentLoopGuard(t.now);
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE; i++) {
			expect(guard.loaded('0:1')).toBe(false);
			t.advance(7);
		}
		expect(guard.loaded('0:1')).toBe(true);
	});

	it('does not count loads that fell out of the window', () => {
		const t = clock();
		const guard = new FragmentLoopGuard(t.now);
		// A rewind past the back buffer loads a fragment again, minutes
		// apart; that is a healthy stream, not a loop.
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE * 3; i++) {
			expect(guard.loaded('0:12')).toBe(false);
			t.advance(FRAGMENT_LOAD_WINDOW_MS);
		}
	});

	it('reads the wall clock when none is given', () => {
		const guard = new FragmentLoopGuard();
		expect(guard.loaded('0:1')).toBe(false);
	});

	it('counts fragments apart from one another', () => {
		const t = clock();
		const guard = new FragmentLoopGuard(t.now);
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE; i++) {
			guard.loaded('0:1');
			guard.loaded('1:1');
		}
		expect(guard.loaded('0:2')).toBe(false);
		expect(guard.loaded('1:1')).toBe(true);
	});
});
