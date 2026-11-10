import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { flagOn, launchFlags } from './flags';

describe('launchFlags', () => {
	it('reads the flags the window was opened with', () => {
		const flags = Object.freeze({ a: false, b: true });
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

/** A window opened with `flags`, typed loosely so a test can hand it
 *  what a broken answer might carry. */
const opened = (flags: Record<string, unknown>) =>
	({ aniGui: { flags } }) as unknown as Parameters<typeof flagOn>[1];

describe('flagOn', () => {
	it('is on only when the window was opened with exactly true for the flag', () => {
		expect(flagOn('a', opened({ a: true }))).toBe(true);
		expect(flagOn('a', opened({ a: false }))).toBe(false);
	});

	it('is off for a flag the set does not hold, and for a window with no set', () => {
		expect(flagOn('a', opened({ b: true }))).toBe(false);
		for (const win of [undefined, {}, { aniGui: {} }, opened({})]) {
			expect(flagOn('a', win)).toBe(false);
		}
	});

	it('is off for any value that only looks like on', () => {
		for (const v of ['true', 'on', 1, 'old', [true], { value: true }, null]) {
			expect(flagOn('a', opened({ a: v })), JSON.stringify(v)).toBe(false);
		}
	});

	it('is on for exactly true and off for every other value, whatever else the set holds', () => {
		fc.assert(
			fc.property(
				fc.string(),
				fc.jsonValue(),
				fc.dictionary(fc.string(), fc.jsonValue()),
				(name, value, rest) => {
					const flags = { ...rest, [name]: value };
					expect(flagOn(name, opened(flags))).toBe(value === true);
				}
			)
		);
	});
});
