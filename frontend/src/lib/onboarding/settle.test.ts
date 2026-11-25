import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SETTLE_MS } from './gate';
import { Settler } from './settle';

let seen: string[][];
let s: Settler;

beforeEach(() => {
	vi.useFakeTimers();
	seen = [];
	s = new Settler((set) => seen.push([...set]), {
		setTimer: (fn, ms) => setTimeout(fn, ms),
		clearTimer: (t) => clearTimeout(t)
	});
});
afterEach(() => {
	s.stop();
	vi.useRealTimers();
});

describe('Settler', () => {
	it('settles a place sighted on screen once SETTLE_MS pass with no other sighting', () => {
		s.sighted('a', true);
		vi.advanceTimersByTime(SETTLE_MS - 1);
		s.sighted('a', true);
		vi.advanceTimersByTime(SETTLE_MS - 1);
		expect(seen).toEqual([]);
		vi.advanceTimersByTime(1);
		expect(seen).toEqual([['a']]);
	});

	it('a sighting off screen or a drop unsettles it, and says so once', () => {
		s.sighted('a', true);
		vi.advanceTimersByTime(SETTLE_MS);
		s.sighted('a', false);
		s.drop('a');
		vi.advanceTimersByTime(SETTLE_MS * 2);
		expect(seen).toEqual([['a'], []]);
	});

	it('keeps places apart, and stop ends every timer', () => {
		s.sighted('a', true);
		vi.advanceTimersByTime(SETTLE_MS / 2);
		s.sighted('b', true);
		vi.advanceTimersByTime(SETTLE_MS / 2);
		expect(seen).toEqual([['a']]);
		s.stop();
		vi.advanceTimersByTime(SETTLE_MS);
		expect(seen).toEqual([['a']]);
	});
});
