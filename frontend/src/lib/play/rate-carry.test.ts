// The playback speed a switch to another show's player carries over.

import { beforeEach, describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { carryRate, takeRate, __resetRateForTests } from './rate-carry';

beforeEach(() => __resetRateForTests());

describe('the carried playback speed', () => {
	it('is handed once to the next player of the show it was carried to', () => {
		carryRate('4013', 1.5);
		expect(takeRate('4013')).toBe(1.5);
		expect(takeRate('4013')).toBeNull();
	});

	it('is dropped by a player of another show', () => {
		carryRate('4013', 1.5);
		expect(takeRate('7158')).toBeNull();
		expect(takeRate('4013')).toBeNull();
	});

	it('carries no normal speed, and no speed that is not one', () => {
		fc.assert(
			fc.property(fc.oneof(fc.constant(1), fc.double({ noNaN: false })), (rate) => {
				__resetRateForTests();
				carryRate('4013', rate);
				const got = takeRate('4013');
				if (rate === 1 || !Number.isFinite(rate) || rate <= 0) expect(got).toBeNull();
				else expect(got).toBe(rate);
			})
		);
	});
});
