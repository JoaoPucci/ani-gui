/**
 * Tests for the semver-ish comparison used by the update notifier.
 *
 * We don't need full semver — our tags are always plain x.y.z and we
 * cut releases manually. The helper:
 *   - Accepts `v0.4.0` or `0.4.0` (strips a leading `v`).
 *   - Compares numerically per dot-segment (so `0.10.0` > `0.4.0`,
 *     not the lexical "0.10" < "0.4" trap).
 *   - Returns false on parse failure (don't fire spurious "update
 *     available" toasts).
 */

import { describe, expect, it } from 'vitest';
import { isNewerVersion } from './version-compare';

describe('isNewerVersion', () => {
	it('returns false when versions are equal', () => {
		expect(isNewerVersion('0.4.0', '0.4.0')).toBe(false);
	});

	it('returns true when the candidate patch is higher', () => {
		expect(isNewerVersion('0.4.0', '0.4.1')).toBe(true);
	});

	it('returns true when the candidate minor is higher', () => {
		expect(isNewerVersion('0.4.5', '0.5.0')).toBe(true);
	});

	it('returns true when the candidate major is higher', () => {
		expect(isNewerVersion('0.9.9', '1.0.0')).toBe(true);
	});

	it('returns false when the candidate is older', () => {
		expect(isNewerVersion('0.4.0', '0.3.9')).toBe(false);
	});

	it('compares segments numerically (avoids lexical 0.10 < 0.4 bug)', () => {
		expect(isNewerVersion('0.9.0', '0.10.0')).toBe(true);
		expect(isNewerVersion('0.10.0', '0.9.0')).toBe(false);
	});

	it('strips a leading v from either side', () => {
		expect(isNewerVersion('v0.4.0', 'v0.4.1')).toBe(true);
		expect(isNewerVersion('0.4.0', 'v0.5.0')).toBe(true);
	});

	it('returns false on parse failure (missing dots, letters)', () => {
		expect(isNewerVersion('0.4.0', 'unstable-build')).toBe(false);
		expect(isNewerVersion('not-a-version', '0.4.0')).toBe(false);
	});

	it('treats extra segments as zeros (0.4 == 0.4.0)', () => {
		expect(isNewerVersion('0.4', '0.4.0')).toBe(false);
		expect(isNewerVersion('0.4', '0.4.1')).toBe(true);
	});
});
