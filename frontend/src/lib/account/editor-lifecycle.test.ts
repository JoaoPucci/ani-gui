import { describe, expect, it } from 'vitest';

import {
	adoptedFlightLanded,
	adoptedLanding,
	flightHolds,
	listingHolds,
	NO_PENDINGS,
	stillHere,
	withPending
} from './editor-lifecycle';
import type { Flight } from './editor-flight';

const self = Symbol('this editor');
const other = Symbol('an editor since unmounted');

describe('flightHolds', () => {
	it('locks while some tracker it listed is still that user', () => {
		const flight: Flight = {
			kind: 'saving',
			listed: { anilist: { user: 'u1' }, mal: { user: 'u2' } }
		};
		expect(flightHolds(flight, (p) => (p === 'mal' ? 'same' : 'other'))).toBe(true);
		expect(flightHolds(flight, () => 'gone')).toBe(false);
	});

	it('locks a flight with no listing, and not one that listed no tracker', () => {
		expect(flightHolds({ kind: 'saving' }, () => 'gone')).toBe(true);
		expect(flightHolds({ kind: 'saving', listed: {} }, () => 'same')).toBe(false);
	});
});

describe('adoptedFlightLanded', () => {
	const flightOf = (owner: symbol): Flight => ({ kind: 'saving', owner });

	it("re-reads when another instance's flight ends on the show on screen", () => {
		expect(
			adoptedFlightLanded(
				{ kitsuId: 'A', flight: flightOf(other) },
				{ kitsuId: 'A', flight: null },
				self
			)
		).toBe(true);
	});

	it('leaves its own flight to the continuation that started it', () => {
		expect(
			adoptedFlightLanded(
				{ kitsuId: 'A', flight: flightOf(self) },
				{ kitsuId: 'A', flight: null },
				self
			)
		).toBe(false);
	});

	it('does nothing when the show changed, or no flight ended', () => {
		const f = flightOf(other);
		expect(
			adoptedFlightLanded({ kitsuId: 'A', flight: f }, { kitsuId: 'B', flight: null }, self)
		).toBe(false);
		expect(
			adoptedFlightLanded({ kitsuId: 'A', flight: f }, { kitsuId: 'A', flight: f }, self)
		).toBe(false);
		expect(
			adoptedFlightLanded({ kitsuId: 'A', flight: null }, { kitsuId: 'A', flight: null }, self)
		).toBe(false);
	});
});

describe('stillHere', () => {
	it('lets a continuation touch its instance only while it is alive and on the same show', () => {
		expect(stillHere({ destroyed: false, kitsuId: 'A' }, 'A')).toBe(true);
		expect(stillHere({ destroyed: true, kitsuId: 'A' }, 'A')).toBe(false);
		expect(stillHere({ destroyed: false, kitsuId: 'B' }, 'A')).toBe(false);
	});
});

describe('withPending', () => {
	it('keeps one pending retry per show, and clears only its own', () => {
		const a = { kitsuId: 'A', intendedStatus: 'paused' as const, intendedProgress: 5 };
		const b = { kitsuId: 'B', intendedStatus: 'dropped' as const, intendedProgress: 2 };
		const both = withPending(withPending(NO_PENDINGS, 'A', { pending: a }), 'B', { pending: b });
		expect(both.get('A')).toEqual({ pending: a });
		const cleared = withPending(both, 'A', null);
		expect(cleared.has('A')).toBe(false);
		expect(cleared.get('B')).toEqual({ pending: b });
		expect(withPending(cleared, 'A', null)).toBe(cleared);
		expect(NO_PENDINGS.size).toBe(0);
	});
});

describe('adoptedLanding', () => {
	const prev = { kitsuId: 'A', flight: { kind: 'saving' as const, owner: other } };
	const next = { kitsuId: 'A', flight: null };

	it('re-reads, and closes a popover showing the sent form', () => {
		expect(adoptedLanding(prev, next, self, prev.flight)).toEqual({ reconcile: true, close: true });
		expect(adoptedLanding(prev, next, self, null)).toEqual({ reconcile: true, close: false });
	});

	it('does neither when nothing landed', () => {
		expect(adoptedLanding(prev, prev, self, prev.flight)).toEqual({
			reconcile: false,
			close: false
		});
	});
});

describe('adoptedLanding and the form on screen', () => {
	const landedFlight: Flight = { kind: 'saving', owner: other };
	const prev = { kitsuId: 'A', flight: landedFlight };
	const next = { kitsuId: 'A', flight: null };

	it('closes only a popover open on the sent form of the flight that landed', () => {
		expect(adoptedLanding(prev, next, self, landedFlight).close).toBe(true);
	});

	it('leaves a form the user opened fresh, or on another flight, alone', () => {
		expect(adoptedLanding(prev, next, self, null)).toEqual({ reconcile: true, close: false });
		const another: Flight = { kind: 'saving', owner: other };
		expect(adoptedLanding(prev, next, self, another)).toEqual({ reconcile: true, close: false });
	});
});

describe('listingHolds', () => {
	const listed = { anilist: { user: 'u1' }, mal: { user: 'm1' } };

	it('holds while some listed tracker is still that user and none is another user', () => {
		expect(listingHolds(listed, () => 'same')).toBe(true);
		expect(listingHolds(listed, (p) => (p === 'anilist' ? 'gone' : 'same'))).toBe(true);
	});

	it('lets go once any listed tracker is another user, or none is still that user', () => {
		expect(listingHolds(listed, (p) => (p === 'anilist' ? 'other' : 'same'))).toBe(false);
		expect(listingHolds(listed, () => 'gone')).toBe(false);
	});

	it('holds with no listing', () => {
		expect(listingHolds(undefined, () => 'other')).toBe(true);
	});
});
