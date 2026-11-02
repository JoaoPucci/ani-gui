import { describe, expect, it } from 'vitest';

import { launchFlags } from './flags';

describe('launchFlags', () => {
	it('reads the flags the window was opened with', () => {
		const flags = Object.freeze({ a: false, b: 'old' });
		expect(launchFlags({ aniGui: { flags } })).toBe(flags);
	});

	it('is an empty, frozen set for a window opened without them', () => {
		for (const win of [undefined, {}, { aniGui: {} }]) {
			const got = launchFlags(win);
			expect(got).toEqual({});
			expect(Object.isFrozen(got)).toBe(true);
		}
	});
});
