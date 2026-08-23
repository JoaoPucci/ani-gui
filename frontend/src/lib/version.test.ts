import { describe, expect, it } from 'vitest';
import { versionLabel } from './version';

describe('versionLabel', () => {
	it('appends -dev so a dev build is visually distinct in the UI', () => {
		expect(versionLabel('0.9.0', true)).toBe('0.9.0-dev');
	});

	it('returns the bare version for a release build', () => {
		expect(versionLabel('0.9.0', false)).toBe('0.9.0');
	});
});
