import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	adoptedFlightLanded,
	adoptedLanding,
	flightHolds,
	listingHolds,
	NO_PENDINGS,
	stillHere,
	withPending,
	type Standing,
	type Watched
} from './editor-lifecycle';
import type { Flight } from './editor-flight';

const self = Symbol('this editor');
const other = Symbol('another editor');
const ids = fc.constantFrom('A', 'B');
const flight: fc.Arbitrary<Flight> = fc.constantFrom(self, other).map((owner) => ({
	kind: 'saving' as const,
	owner
}));
const watched: fc.Arbitrary<Watched> = fc.record({
	kitsuId: ids,
	flight: fc.option(flight, { nil: null })
});
const standing = fc.constantFrom<Standing>('same', 'other', 'gone');
const providers = ['anilist', 'mal', 'inhouse'];
const listing = fc.dictionary(fc.constantFrom(...providers), fc.record({ user: fc.string() }), {
	maxKeys: 3
});
const standings = fc.dictionary(fc.constantFrom(...providers), standing);

describe('adoptedLanding, by property', () => {
	it("re-reads exactly when another instance's flight ended on the same show", () => {
		fc.assert(
			fc.property(watched, watched, (prev, next) => {
				const landed = adoptedFlightLanded(prev, next, self);
				expect(landed).toBe(
					prev.kitsuId === next.kitsuId &&
						prev.flight !== null &&
						next.flight === null &&
						prev.flight.owner !== self
				);
				expect(adoptedLanding(prev, next, self, null).reconcile).toBe(landed);
			})
		);
	});

	it('closes only on a landing, and only on the form of the flight that landed', () => {
		// The popover shows the flight that was watched, another flight, or a
		// fresh form.
		const scene = fc
			.tuple(watched, watched, flight)
			.chain(([prev, next, another]) =>
				fc.tuple(fc.constant(prev), fc.constant(next), fc.constantFrom(prev.flight, another, null))
			);
		fc.assert(
			fc.property(scene, ([prev, next, shown]) => {
				const { reconcile, close } = adoptedLanding(prev, next, self, shown);
				if (close) {
					expect(reconcile).toBe(true);
					expect(shown).toBe(prev.flight);
				}
				if (reconcile && shown !== null && shown === prev.flight) expect(close).toBe(true);
			})
		);
	});

	it('closes on every landing of the flight whose form is shown', () => {
		fc.assert(
			fc.property(ids, (id) => {
				const landed: Flight = { kind: 'saving', owner: other };
				expect(
					adoptedLanding(
						{ kitsuId: id, flight: landed },
						{ kitsuId: id, flight: null },
						self,
						landed
					)
				).toEqual({ reconcile: true, close: true });
			})
		);
	});
});

describe('stillHere, by property', () => {
	it('is alive and on the same show, nothing else', () => {
		fc.assert(
			fc.property(fc.boolean(), ids, ids, (destroyed, on, id) => {
				expect(stillHere({ destroyed, kitsuId: on }, id)).toBe(!destroyed && on === id);
			})
		);
	});
});

describe('the account rules, by property', () => {
	it('a flight locks while some listed tracker is still that user', () => {
		fc.assert(
			fc.property(listing, standings, (listed, now) => {
				const read = (p: string) => now[p] ?? 'gone';
				const holds = flightHolds({ kind: 'saving', listed }, read);
				expect(holds).toBe(Object.keys(listed).some((p) => read(p) === 'same'));
			})
		);
	});

	it('a retry holds while some listed tracker is that user and none is another user', () => {
		fc.assert(
			fc.property(listing, standings, (listed, now) => {
				const read = (p: string) => now[p] ?? 'gone';
				const seen = Object.keys(listed).map(read);
				expect(listingHolds(listed, read)).toBe(seen.includes('same') && !seen.includes('other'));
			})
		);
	});

	it('a retry that holds is a flight that locks, never the other way round', () => {
		fc.assert(
			fc.property(listing, standings, (listed, now) => {
				const read = (p: string) => now[p] ?? 'gone';
				if (listingHolds(listed, read))
					expect(flightHolds({ kind: 'saving', listed }, read)).toBe(true);
			})
		);
	});

	it('no listing holds and locks', () => {
		fc.assert(
			fc.property(standing, (s) => {
				expect(listingHolds(undefined, () => s)).toBe(true);
				expect(flightHolds({ kind: 'saving' }, () => s)).toBe(true);
			})
		);
	});
});

describe('withPending, by property', () => {
	it('holds, per show, the last retry set, and none once cleared', () => {
		const step = fc.tuple(ids, fc.option(fc.integer({ min: 0, max: 24 }), { nil: null }));
		fc.assert(
			fc.property(fc.array(step, { maxLength: 10 }), (run) => {
				let pendings = NO_PENDINGS;
				const model = new Map<string, number>();
				for (const [id, progress] of run) {
					const pending =
						progress === null
							? null
							: {
									pending: {
										kitsuId: id,
										intendedStatus: 'paused' as const,
										intendedProgress: progress
									}
								};
					pendings = withPending(pendings, id, pending);
					if (progress === null) model.delete(id);
					else model.set(id, progress);
				}
				for (const id of ['A', 'B']) {
					expect(pendings.get(id)?.pending.intendedProgress).toBe(model.get(id));
				}
				expect(NO_PENDINGS.size).toBe(0);
			})
		);
	});
});
