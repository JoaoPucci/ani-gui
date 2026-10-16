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
		for (const reason of ['unanswered', 'past_affinity', 'superseded'] as const) {
			expect(establishesNothing({ unsettled: reason }), reason).toBe(true);
			expect(availabilityVerdict({ available: false, unsettled: reason }), reason).toBeNull();
		}
	});

	it('a refusal by name is the verdict that the catalogue does not carry the show', () => {
		// The probe reached the provider, and the provider lists nothing
		// named for the show: Play can only fail, so every surface reads
		// it as the show not being in the catalogue. The backend persists
		// nothing, so the next look asks again.
		expect(establishesNothing({ unsettled: 'refused' })).toBe(false);
		expect(availabilityVerdict({ available: false, unsettled: 'refused' })).toBe(false);
		// Whatever `available` says beside it.
		expect(availabilityVerdict({ available: true, unsettled: 'refused' })).toBe(false);
	});
});
