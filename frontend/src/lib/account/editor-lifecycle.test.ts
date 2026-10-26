import { describe, expect, it } from 'vitest';

import {
	adoptedFlightLanded,
	flightHolds,
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
		expect(flightHolds(flight, (p) => p === 'mal')).toBe(true);
		expect(flightHolds(flight, () => false)).toBe(false);
	});

	it('locks a flight with no listing, and not one that listed no tracker', () => {
		expect(flightHolds({ kind: 'saving' }, () => false)).toBe(true);
		expect(flightHolds({ kind: 'saving', listed: {} }, () => true)).toBe(false);
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
		const both = withPending(withPending(NO_PENDINGS, 'A', a), 'B', b);
		expect(both.get('A')).toEqual(a);
		const cleared = withPending(both, 'A', null);
		expect(cleared.has('A')).toBe(false);
		expect(cleared.get('B')).toEqual(b);
		expect(withPending(cleared, 'A', null)).toBe(cleared);
		expect(NO_PENDINGS.size).toBe(0);
	});
});
