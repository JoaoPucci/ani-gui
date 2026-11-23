import { describe, expect, it } from 'vitest';
import { RecoveryResume } from './resume-after-recovery';

// The key is (show, episode): the episode changes within one page,
// and a switch to another show's player mounts a new page that takes
// what the old one left, so both dimensions can change while a capture
// is pending. Signatures updated in this red commit with
// the contract: episode-only keying let show B's attach consume show
// A's position whenever their episode numbers matched.

describe('RecoveryResume', () => {
	it('hands the position to the same show and episode exactly once', () => {
		const r = new RecoveryResume();
		r.capture('show-a', 6, 432.5);
		expect(r.consume('show-a', 6)).toBe(432.5);
		// The next attach (quality change, navigation back) starts
		// fresh — the capture was for the recovery landing only.
		expect(r.consume('show-a', 6)).toBeNull();
	});

	it('discards the capture when a different episode attaches', () => {
		const r = new RecoveryResume();
		r.capture('show-a', 6, 432.5);
		expect(r.consume('show-a', 7)).toBeNull();
		expect(r.consume('show-a', 6)).toBeNull();
	});

	it('discards the capture when a different show attaches', () => {
		// Same-route navigation can swap the show while keeping the
		// episode number; show B's attach is not the recovery landing
		// and must not inherit show A's position.
		const r = new RecoveryResume();
		r.capture('show-a', 6, 432.5);
		expect(r.consume('show-b', 6)).toBeNull();
		expect(r.consume('show-a', 6)).toBeNull();
	});

	it('does not carry near-zero positions', () => {
		const r = new RecoveryResume();
		r.capture('show-a', 6, 0.4);
		expect(r.consume('show-a', 6)).toBeNull();
	});

	it('a later capture replaces the earlier one', () => {
		const r = new RecoveryResume();
		r.capture('show-a', 6, 100);
		r.capture('show-a', 6, 200);
		expect(r.consume('show-a', 6)).toBe(200);
	});
});

describe('RecoveryResume — a switch to the picked show', () => {
	it('carries the point it was asked for, zero included', () => {
		// "Switch to it now" at 0:00 says it picks up there: zero is a
		// point, not the absence of one, so the pick's own saved position
		// does not take its place.
		const r = new RecoveryResume();
		r.carry('4013', 2, 0);
		expect(r.consume('4013', 2)).toBe(0);
		r.carry('4013', 2, 0.4);
		expect(r.consume('4013', 2)).toBe(0.4);
		r.carry('4013', 2, 74);
		expect(r.consume('4013', 2)).toBe(74);
	});

	it('carries nothing for a point that is no number of seconds', () => {
		const r = new RecoveryResume();
		r.carry('4013', 2, Number.NaN);
		expect(r.consume('4013', 2)).toBeNull();
		r.carry('4013', 2, -1);
		expect(r.consume('4013', 2)).toBeNull();
	});
});

describe('RecoveryResume — forgetting with the history', () => {
	it("forgets a capture for the show whose positions are forgotten, and only that show's", () => {
		const r = new RecoveryResume();
		r.capture('show-a', 6, 432.5);
		r.forgetShow('show-b');
		expect(r.consume('show-a', 6)).toBe(432.5);
		r.capture('show-a', 6, 432.5);
		r.forgetShow('show-a');
		expect(r.consume('show-a', 6)).toBeNull();
	});

	it('forgets any capture when every position is forgotten', () => {
		const r = new RecoveryResume();
		r.capture('show-a', 6, 432.5);
		r.forgetAll();
		expect(r.consume('show-a', 6)).toBeNull();
	});
});
