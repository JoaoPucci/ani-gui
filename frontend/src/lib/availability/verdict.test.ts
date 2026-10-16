import { describe, expect, it } from 'vitest';
import { availabilityVerdict, establishesNothing } from './verdict';

describe('what an availability answer establishes', () => {
	it('a settled answer is its verdict', () => {
		expect(establishesNothing({})).toBe(false);
		expect(availabilityVerdict({ available: true })).toBe(true);
		expect(availabilityVerdict({ available: false })).toBe(false);
	});

	it('an unsettled answer establishes nothing, whatever it says of availability', () => {
		expect(establishesNothing({ unsettled: true })).toBe(true);
		expect(availabilityVerdict({ available: false, unsettled: true })).toBeNull();
	});
});
