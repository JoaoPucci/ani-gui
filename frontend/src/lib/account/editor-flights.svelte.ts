// The list editor's writes in flight and pending retries, kept for the
// life of the app rather than of an editor instance: the detail page
// unmounts its editor whenever it leaves a show, while a save can wait
// out a tracker's rate limit for minutes. The rules are pure, in
// `editor-flight.ts` and `editor-lifecycle.ts`; this holds the state.

import type { Provider } from './types';
import { followAccount } from './held-account';
import {
	flightFor,
	NO_FLIGHTS,
	withFlight,
	withoutFlight,
	type Flight,
	type Flights
} from './editor-flight';
import { flightHolds, NO_PENDINGS, withPending, type Pendings } from './editor-lifecycle';
import type { PendingEdit } from './list-entry-pending';

class EditorFlightStore {
	flights = $state.raw<Flights>(NO_FLIGHTS);
	pendings = $state.raw<Pendings>(NO_PENDINGS);

	/** The write in flight that still locks `kitsuId`'s editor: one whose
	 *  trackers are still connected as the users it listed. Reactive to
	 *  the account store. */
	locking(kitsuId: string): Flight | null {
		const flight = flightFor(this.flights, kitsuId);
		if (!flight) return null;
		const holds = flightHolds(flight, (provider, listed) =>
			followAccount(provider as Provider, listed)()
		);
		return holds ? flight : null;
	}

	begin(kitsuId: string, flight: Flight): void {
		this.flights = withFlight(this.flights, kitsuId, flight);
	}

	/** `flight` settled; a newer one recorded for the show stays. */
	end(kitsuId: string, flight: Flight): void {
		this.flights = withoutFlight(this.flights, kitsuId, flight);
	}

	pendingFor(kitsuId: string): PendingEdit | null {
		return this.pendings.get(kitsuId) ?? null;
	}

	setPending(kitsuId: string, pending: PendingEdit | null): void {
		this.pendings = withPending(this.pendings, kitsuId, pending);
	}
}

export const editorFlights = new EditorFlightStore();
