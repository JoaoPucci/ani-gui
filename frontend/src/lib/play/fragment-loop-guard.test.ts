import { describe, expect, it } from 'vitest';
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
});
