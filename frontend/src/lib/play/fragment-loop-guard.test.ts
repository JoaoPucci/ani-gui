import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	FRAGMENT_LOAD_ALLOWANCE,
	FRAGMENT_LOAD_WINDOW_MS,
	FragmentLoopGuard,
	fragmentLoopKey,
	type FragmentIdentity
} from './fragment-loop-guard';

/** hls.js's fragment identity as the page hands it over. */
function frag(type: string, level: number, sn: number): FragmentIdentity {
	return { type, level, sn } as FragmentIdentity;
}

function clock(start = 0) {
	let now = start;
	return {
		now: () => now,
		advance(ms: number) {
			now += ms;
		}
	};
}

describe('fragmentLoopKey', () => {
	it('tells renditions apart: main, audio and subtitle playlists number their fragments independently', () => {
		const main = fragmentLoopKey(frag('main', 0, 12));
		const audio = fragmentLoopKey(frag('audio', 0, 12));
		const subtitle = fragmentLoopKey(frag('subtitle', 0, 12));
		expect(new Set([main, audio, subtitle]).size).toBe(3);
	});

	it('tells levels and sequence numbers apart within a rendition', () => {
		expect(fragmentLoopKey(frag('main', 0, 12))).not.toBe(fragmentLoopKey(frag('main', 1, 12)));
		expect(fragmentLoopKey(frag('main', 0, 12))).not.toBe(fragmentLoopKey(frag('main', 0, 13)));
	});

	it('is the same for the same fragment', () => {
		expect(fragmentLoopKey(frag('main', 0, 12))).toBe(fragmentLoopKey(frag('main', 0, 12)));
	});
});

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
		// The same fragment loaded again at a healthy pace, as a viewer
		// coming back to one spot every so often loads it; that is not a
		// loop.
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE * 3; i++) {
			expect(guard.loaded('0:12')).toBe(false);
			t.advance(FRAGMENT_LOAD_WINDOW_MS);
		}
	});

	it('trips once: the engine it stops is stopped, and loads that follow are not a second trip', () => {
		const t = clock();
		const guard = new FragmentLoopGuard(t.now);
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE; i++) guard.loaded('main:0:1');
		expect(guard.loaded('main:0:1')).toBe(true);
		expect(guard.loaded('main:0:1')).toBe(false);
		expect(guard.loaded('main:0:2')).toBe(false);
	});

	it('says whether it has tripped, for whoever else would restart the engine', () => {
		const t = clock();
		const guard = new FragmentLoopGuard(t.now);
		expect(guard.hasTripped).toBe(false);
		for (let i = 0; i <= FRAGMENT_LOAD_ALLOWANCE; i++) guard.loaded('main:0:1');
		expect(guard.hasTripped).toBe(true);
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

	it('does not add up healthy loads across renditions', () => {
		// Two seeks within the window reload the main, audio and
		// subtitle fragments that cover the spot: six loads sharing a
		// level and sequence number, two per rendition. Six is past the
		// allowance if the renditions are added up, and two is not when
		// they are kept apart.
		const t = clock();
		const guard = new FragmentLoopGuard(t.now);
		for (let i = 0; i < 2; i++) {
			for (const type of ['main', 'audio', 'subtitle']) {
				expect(guard.loaded(fragmentLoopKey(frag(type, 0, 12)))).toBe(false);
			}
		}
	});

	it('forgets the loads of a fragment the engine let go of behind the playhead', () => {
		// The engine keeps a bounded stretch behind the playhead. A viewer
		// going back and forth between two spots further apart than that
		// loads the far spot's fragment again on every return, a couple of
		// seconds apart: each load follows a release that took it out of
		// the buffer, so each is the first since it was let go.
		const t = clock();
		const guard = new FragmentLoopGuard(t.now);
		// The fragment starts at 8 s; the trim reaches 60 s.
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE * 3; i++) {
			expect(guard.loaded('main:0:3', 8)).toBe(false);
			guard.released(60);
			t.advance(500);
		}
		expect(guard.hasTripped).toBe(false);
	});

	it('keeps counting a fragment the release did not reach', () => {
		// A fragment that never reaches the buffer sits at the playhead,
		// never behind it: a release up to an earlier point leaves its
		// count alone, and the loop still trips.
		const t = clock();
		const guard = new FragmentLoopGuard(t.now);
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE; i++) {
			expect(guard.loaded('main:0:20', 100)).toBe(false);
			guard.released(60);
		}
		expect(guard.loaded('main:0:20', 100)).toBe(true);
	});

	it('releases a fragment the trim reached any part of: one starting before the released point', () => {
		// The engine drops a fragment from its record once any of it has
		// left the buffer, and loads the whole of it again; the one the
		// trim point falls inside is the one a viewer just past the back
		// buffer reloads.
		const t = clock();
		const straddling = new FragmentLoopGuard(t.now);
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE * 3; i++) {
			expect(straddling.loaded('main:0:4', 28)).toBe(false);
			straddling.released(30);
		}

		const at = new FragmentLoopGuard(t.now);
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE; i++) at.loaded('main:0:5', 30);
		at.released(30);
		expect(at.loaded('main:0:5', 30)).toBe(true);
	});

	it('trips on any burst of loads with no release that reaches the fragment', () => {
		// Property: whatever releases come in between, a fragment loaded
		// past the allowance inside the window, with every release at or
		// before its start, trips the guard.
		fc.assert(
			fc.property(
				fc.double({ min: 1, max: 10_000, noNaN: true }),
				fc.array(fc.double({ min: 0.001, max: 100, noNaN: true }), {
					minLength: FRAGMENT_LOAD_ALLOWANCE + 1,
					maxLength: FRAGMENT_LOAD_ALLOWANCE + 1
				}),
				(start, shortBy) => {
					const t = clock();
					const guard = new FragmentLoopGuard(t.now);
					let tripped = false;
					for (const short of shortBy) {
						tripped = guard.loaded('main:0:9', start) || tripped;
						guard.released(start - short);
						t.advance(1);
					}
					return tripped;
				}
			)
		);
	});

	it('never trips on a fragment released after every load', () => {
		fc.assert(
			fc.property(
				fc.double({ min: 0, max: 10_000, noNaN: true }),
				fc.integer({ min: 1, max: 50 }),
				fc.integer({ min: 0, max: 2_000 }),
				fc.double({ min: 0.001, max: 100, noNaN: true }),
				(start, loads, gapMs, past) => {
					const t = clock();
					const guard = new FragmentLoopGuard(t.now);
					for (let i = 0; i < loads; i++) {
						if (guard.loaded('main:0:9', start)) return false;
						guard.released(start + past);
						t.advance(gapMs);
					}
					return !guard.hasTripped;
				}
			)
		);
	});
});
