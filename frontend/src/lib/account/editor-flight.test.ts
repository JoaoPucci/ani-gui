import { describe, expect, it } from 'vitest';

import { flightFor, NO_FLIGHTS, reopenForm, withFlight, withoutFlight } from './editor-flight';

describe("the editor's writes in flight, per show", () => {
	it('locks the show whose write is out, and no other', () => {
		const flights = withFlight(NO_FLIGHTS, 'A', {
			kind: 'saving',
			sent: { status: 'paused', progress: 5 }
		});
		expect(flightFor(flights, 'A')).toEqual({
			kind: 'saving',
			sent: { status: 'paused', progress: 5 }
		});
		expect(flightFor(flights, 'B')).toBeNull();
	});

	it('settles one show without touching another, and never mutates the map it was given', () => {
		const both = withFlight(withFlight(NO_FLIGHTS, 'A', { kind: 'saving' }), 'B', {
			kind: 'removing'
		});
		const settled = withoutFlight(both, 'A');
		expect(flightFor(settled, 'A')).toBeNull();
		expect(flightFor(settled, 'B')).toEqual({ kind: 'removing' });
		expect(flightFor(both, 'A')).toEqual({ kind: 'saving' });
		expect(withoutFlight(settled, 'nothing')).toBe(settled);
		expect(NO_FLIGHTS.size).toBe(0);
	});
});

describe('reopenForm', () => {
	it("restores the save's form with the status it was seeded on, not another show's", () => {
		// A → B → A: the editor seeded B in between; reopening A must put
		// back A's seed, or a retry reads a progress-only save as a status
		// write.
		expect(
			reopenForm({
				kind: 'saving',
				sent: { status: 'watching', progress: 4, seededStatus: 'watching' }
			})
		).toEqual({ status: 'watching', progress: 4, seededStatus: 'watching' });
	});

	it('has no form for a removal', () => {
		expect(reopenForm({ kind: 'removing' })).toBeNull();
	});
});
