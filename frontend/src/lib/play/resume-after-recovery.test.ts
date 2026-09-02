import { describe, expect, it } from 'vitest';
import { RecoveryResume } from './resume-after-recovery';

describe('RecoveryResume', () => {
	it('hands the position to the same episode exactly once', () => {
		const r = new RecoveryResume();
		r.capture(6, 432.5);
		expect(r.consume(6)).toBe(432.5);
		// The next attach (quality change, navigation back) starts
		// fresh — the capture was for the recovery landing only.
		expect(r.consume(6)).toBeNull();
	});

	it('discards the capture when a different episode attaches', () => {
		// The user picked another episode while the recovery was in
		// flight; their old position must not seek into it — and it
		// must not linger for a later return either.
		const r = new RecoveryResume();
		r.capture(6, 432.5);
		expect(r.consume(7)).toBeNull();
		expect(r.consume(6)).toBeNull();
	});

	it('does not carry near-zero positions', () => {
		// Restarting from 0.4s is indistinguishable from a restart;
		// the seek would be noise.
		const r = new RecoveryResume();
		r.capture(6, 0.4);
		expect(r.consume(6)).toBeNull();
	});

	it('a later capture replaces the earlier one', () => {
		const r = new RecoveryResume();
		r.capture(6, 100);
		r.capture(6, 200);
		expect(r.consume(6)).toBe(200);
	});
});
