import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	flightFor,
	NO_FLIGHTS,
	reopenForm,
	withFlight,
	withoutFlight,
	type Flight,
	type Flights
} from './editor-flight';

const ids = fc.constantFrom('A', 'B', 'C');
const status = fc.constantFrom('planning', 'watching', 'paused', 'dropped', 'completed' as const);
/** Each a fresh object: flights are told apart by identity. */
const flight: fc.Arbitrary<Flight> = fc
	.oneof(
		fc.constant({ kind: 'removing' as const }),
		fc.record({
			kind: fc.constant('saving' as const),
			sent: fc.record(
				{ status, progress: fc.integer({ min: 0, max: 24 }), seededStatus: status },
				{ requiredKeys: ['status', 'progress'] }
			)
		})
	)
	.map((f): Flight => ({ ...f }));
/** A run of recordings and settlements, as editors make them. */
const steps = fc.array(fc.tuple(fc.boolean(), ids, flight), { maxLength: 12 });

function replay(run: [boolean, string, Flight][]): {
	flights: Flights;
	model: Map<string, Flight>;
} {
	let flights: Flights = NO_FLIGHTS;
	const model = new Map<string, Flight>();
	for (const [begin, id, f] of run) {
		if (begin) {
			flights = withFlight(flights, id, f);
			model.set(id, f);
		} else {
			// Settle whatever is recorded for the show, by identity.
			const current = flightFor(flights, id) ?? f;
			flights = withoutFlight(flights, id, current);
			if (model.get(id) === current) model.delete(id);
		}
	}
	return { flights, model };
}

describe('the editor flight map, by property', () => {
	it('holds, per show, the last flight recorded and not since settled', () => {
		fc.assert(
			fc.property(steps, (run) => {
				const { flights, model } = replay(run);
				for (const id of ['A', 'B', 'C'])
					expect(flightFor(flights, id)).toBe(model.get(id) ?? null);
			})
		);
	});

	it('settling another flight than the one recorded changes nothing', () => {
		fc.assert(
			fc.property(steps, ids, flight, (run, id, stranger) => {
				const { flights } = replay(run);
				expect(withoutFlight(flights, id, stranger)).toBe(flights);
			})
		);
	});

	it('never changes the map it is given', () => {
		fc.assert(
			fc.property(steps, ids, flight, (run, id, f) => {
				const { flights } = replay(run);
				const before = new Map(flights);
				withFlight(flights, id, f);
				withoutFlight(flights, id);
				expect(new Map(flights)).toEqual(before);
			})
		);
	});
});

describe('reopenForm, by property', () => {
	it('gives back exactly what a save sent, and nothing for a removal', () => {
		fc.assert(
			fc.property(flight, (f) => {
				const form = reopenForm(f);
				if (f.sent) expect(form).toEqual(f.sent);
				else expect(form).toBeNull();
			})
		);
	});
});
