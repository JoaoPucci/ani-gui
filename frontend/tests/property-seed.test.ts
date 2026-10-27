import fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { pinnedSeed } from './property-seed';

describe('the property seed', () => {
	it('is the whole number the environment names', () => {
		expect(pinnedSeed({ FAST_CHECK_SEED: '20261026' })).toBe(20261026);
		expect(pinnedSeed({ FAST_CHECK_SEED: '-3' })).toBe(-3);
	});

	it('is none when the environment names none, or something else', () => {
		expect(pinnedSeed({})).toBeUndefined();
		expect(pinnedSeed({ FAST_CHECK_SEED: '' })).toBeUndefined();
		expect(pinnedSeed({ FAST_CHECK_SEED: 'seven' })).toBeUndefined();
		expect(pinnedSeed({ FAST_CHECK_SEED: '1.5' })).toBeUndefined();
	});

	it('is the seed fast-check runs with, in this run', () => {
		// The setup file has run: a run that names a seed, as the
		// coverage run does, generates from it; one that names none
		// keeps fast-check's fresh seed.
		expect(fc.readConfigureGlobal().seed).toBe(pinnedSeed(process.env));
	});
});
