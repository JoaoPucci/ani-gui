import { describe, expect, it } from 'vitest';
import type { Config } from '$lib/api';
import { pickAvailabilityMode } from './mode';

/** Tiny fixture builder — the helper only reads `mode`, so the
 *  other fields are irrelevant for this test. The cast keeps the
 *  type contract honest without forcing the test to spell out
 *  every Config field. */
function cfg(mode: string): Config {
	return { mode } as unknown as Config;
}

describe('pickAvailabilityMode', () => {
	it('returns "dub" when config.mode is exactly "dub"', () => {
		expect(pickAvailabilityMode(cfg('dub'))).toBe('dub');
	});

	it('defaults to "sub" when config is null (settings not yet loaded)', () => {
		// First paint after mount but before settingsGet resolves.
		// Without a default the filter call site would crash on
		// `'sub' | 'dub'` typing — defaulting to 'sub' matches what
		// the rest of the app already does inline.
		expect(pickAvailabilityMode(null)).toBe('sub');
		expect(pickAvailabilityMode(undefined)).toBe('sub');
	});

	it('defaults to "sub" when mode is "sub" (explicit pin against accidental drift)', () => {
		expect(pickAvailabilityMode(cfg('sub'))).toBe('sub');
	});

	it('defaults to "sub" when mode is any other string (defensive)', () => {
		// Config.mode is typed as a wider string upstream; an unknown
		// value (corrupt TOML, future addition) shouldn't surface as
		// 'dub' by accident.
		expect(pickAvailabilityMode(cfg(''))).toBe('sub');
		expect(pickAvailabilityMode(cfg('raw'))).toBe('sub');
	});
});
