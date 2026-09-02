import { describe, expect, it } from 'vitest';
import { RecoveryResume } from './resume-after-recovery';

// The key is (show, episode): the /play route's component is reused
// across same-route navigation, so both dimensions can change while
// a capture is pending. Signatures updated in this red commit with
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
