// @vitest-environment happy-dom
//
// The one place the app asks whether the user wants motion. Everything
// that moves reads `reducedMotion`; this file pins how it reads the
// system and how it follows a change made while the app is open.
import { describe, expect, it, vi } from 'vitest';
import {
	REDUCED_MOTION_QUERY,
	createReducedMotion,
	reducedMotion,
	systemMotionQuery,
	type MotionQuery
} from './reduced-motion.svelte';

/** A media query list the test flips by hand. */
function fakeQuery(matches: boolean) {
	const listeners = new Set<() => void>();
	const query: { -readonly [K in keyof MotionQuery]: MotionQuery[K] } & {
		flip(next: boolean): void;
		listenerCount(): number;
	} = {
		matches,
		addEventListener: (_type, listener) => listeners.add(listener),
		removeEventListener: (_type, listener) => listeners.delete(listener),
		flip(next) {
			query.matches = next;
			for (const l of [...listeners]) l();
		},
		listenerCount: () => listeners.size
	};
	return query;
}

describe('createReducedMotion', () => {
	it('reads the preference the system reports at start', () => {
		expect(createReducedMotion(fakeQuery(true)).current).toBe(true);
		expect(createReducedMotion(fakeQuery(false)).current).toBe(false);
	});

	it('reads full motion where there is no system to ask', () => {
		expect(createReducedMotion(null).current).toBe(false);
	});

	it('follows a change made while the app is open', () => {
		const query = fakeQuery(false);
		const motion = createReducedMotion(query);
		query.flip(true);
		expect(motion.current).toBe(true);
		query.flip(false);
		expect(motion.current).toBe(false);
	});

	it('tells each listener about a change until it unsubscribes', () => {
		const query = fakeQuery(false);
		const motion = createReducedMotion(query);
		const seen: boolean[] = [];
		const stop = motion.onChange((reduced) => seen.push(reduced));
		query.flip(true);
		stop();
		query.flip(false);
		expect(seen).toEqual([true]);
	});

	it('listens to the system once, however many listeners it has', () => {
		const query = fakeQuery(false);
		const motion = createReducedMotion(query);
		motion.onChange(() => {});
		motion.onChange(() => {});
		expect(query.listenerCount()).toBe(1);
	});

	it('hands back a no-op unsubscribe where there is no system to ask', () => {
		const motion = createReducedMotion(null);
		const listener = vi.fn();
		const stop = motion.onChange(listener);
		expect(() => stop()).not.toThrow();
		expect(listener).not.toHaveBeenCalled();
	});
});

describe('systemMotionQuery', () => {
	it('asks the window for the reduced-motion media query', () => {
		const query = fakeQuery(true);
		const matchMedia = vi.fn(() => query);
		expect(systemMotionQuery({ matchMedia })).toBe(query);
		expect(matchMedia).toHaveBeenCalledWith(REDUCED_MOTION_QUERY);
	});

	it('answers nothing without a window', () => {
		expect(systemMotionQuery(undefined)).toBeNull();
	});

	it('answers nothing from a window that cannot match media', () => {
		expect(systemMotionQuery({})).toBeNull();
	});

	it('names the standard media feature', () => {
		expect(REDUCED_MOTION_QUERY).toBe('(prefers-reduced-motion: reduce)');
	});
});

describe('reducedMotion', () => {
	it('is read from the window the app runs in', () => {
		// happy-dom reports no preference, as a desktop with motion on does.
		expect(reducedMotion.current).toBe(window.matchMedia(REDUCED_MOTION_QUERY).matches);
	});
});
