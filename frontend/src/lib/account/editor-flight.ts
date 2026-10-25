// The list editor's writes in flight, kept per show. The detail route
// reuses one editor across shows, so a save still out for one show —
// waiting out a tracker's rate limit, say — must lock that show's
// editor and no other. Pure: the component holds the map and swaps it.

import type { ListStatus } from './types';

/** One show's write in flight: which kind, and for a save what it sent,
 *  so a reopened editor can show it. */
export interface Flight {
	kind: 'saving' | 'removing';
	sent?: { status: ListStatus; progress: number; seededStatus?: ListStatus };
}

/** The form a reopened editor shows for the write in flight: what the
 *  save sent, and the status the form was seeded on when it did. `null`
 *  for a write that sent no form (a removal). */
export function reopenForm(
	flight: Flight
): { status: ListStatus; progress: number; seededStatus?: ListStatus } | null {
	return flight.sent ? { status: flight.sent.status, progress: flight.sent.progress } : null;
}

export type Flights = ReadonlyMap<string, Flight>;

export const NO_FLIGHTS: Flights = new Map();

/** The write in flight for `kitsuId`, if any. */
export function flightFor(flights: Flights, kitsuId: string): Flight | null {
	return flights.get(kitsuId) ?? null;
}

/** `flights` with `kitsuId`'s write recorded. */
export function withFlight(flights: Flights, kitsuId: string, flight: Flight): Flights {
	const next = new Map(flights);
	next.set(kitsuId, flight);
	return next;
}

/** `flights` with `kitsuId`'s write settled. */
export function withoutFlight(flights: Flights, kitsuId: string): Flights {
	if (!flights.has(kitsuId)) return flights;
	const next = new Map(flights);
	next.delete(kitsuId);
	return next;
}
