import { describe, expect, it } from 'vitest';

import { ctaState } from './cta-state';

describe('ctaState', () => {
	it('shows the loading skeleton only while the probe is in flight', () => {
		expect(ctaState(false, null)).toBe('loading');
	});

	it('shows the not-in-catalogue notice for a negative verdict', () => {
		expect(ctaState(true, false)).toBe('unavailable');
	});

	it('shows the real actions for a positive verdict', () => {
		expect(ctaState(true, true)).toBe('ready');
	});

	it('falls back to the real actions when the probe settled with an error', () => {
		// The load-bearing case: check_availability surfaces throttled /
		// failed allmanga searches as an error (by design — a transient
		// failure must not poison the cache with available:false), and
		// the old template mapped that null verdict to the same pulsing
		// skeleton as an in-flight probe. With no retry anywhere, the
		// play-button area loaded forever. A settled-but-unknown verdict
		// must render the actions instead; the lazy click path already
		// surfaces the real error if the show truly can't stream.
		expect(ctaState(true, null)).toBe('ready');
	});
});
