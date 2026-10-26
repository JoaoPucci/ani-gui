// The list editor's writes in flight, kept per show. The detail route
// reuses one editor across shows, so a save still out for one show —
// waiting out a tracker's rate limit, say — must lock that show's
// editor and no other. Pure: the app-lifetime store
// `editor-flights.svelte.ts` holds the map and swaps it.

import type { ListStatus } from './types';
import type { ListedAccount } from './held-account';

/** One show's write in flight: which kind, and for a save what it sent,
 *  so a reopened editor can show it. */
export interface Flight {
	kind: 'saving' | 'removing';
	sent?: { status: ListStatus; progress: number; seededStatus?: ListStatus };
	/** The editor instance that started it. */
	owner?: symbol;
	/** The account each tracker was listed under when the write's fan-out
	 *  listed them. */
	listed?: Readonly<Record<string, ListedAccount>>;
}

/** The form a reopened editor shows for the write in flight: what the
 *  save sent, and the status the form was seeded on when it did. `null`
 *  for a write that sent no form (a removal). */
export function reopenForm(
	flight: Flight
): { status: ListStatus; progress: number; seededStatus?: ListStatus } | null {
	return flight.sent ? { ...flight.sent } : null;
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

/** `flights` with `kitsuId`'s write settled. Given the `flight` that
 *  settled, only that one: a newer write recorded for the show since is
 *  left in place. */
export function withoutFlight(flights: Flights, kitsuId: string, flight?: Flight): Flights {
	if (!flights.has(kitsuId)) return flights;
	if (flight && flights.get(kitsuId) !== flight) return flights;
	const next = new Map(flights);
	next.delete(kitsuId);
	return next;
}
