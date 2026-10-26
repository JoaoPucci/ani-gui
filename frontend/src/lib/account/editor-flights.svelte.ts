// The list editor's writes in flight and pending retries, kept for the
// life of the app rather than of an editor instance: the detail page
// unmounts its editor whenever it leaves a show, while a save can wait
// out a tracker's rate limit for minutes. The rules are pure, in
// `editor-flight.ts` and `editor-lifecycle.ts`; this holds the state.

import type { Provider } from './types';
import { followAccount, type ListedAccount } from './held-account';
import {
	flightFor,
	NO_FLIGHTS,
	withFlight,
	withoutFlight,
	type Flight,
	type Flights
} from './editor-flight';
import {
	flightHolds,
	listingHolds,
	NO_PENDINGS,
	withPending,
	type Pendings
} from './editor-lifecycle';
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

	/** The write in flight for `kitsuId`, whether or not it still locks:
	 *  what an instance watches for the write to settle. */
	flightOf(kitsuId: string): Flight | null {
		return flightFor(this.flights, kitsuId);
	}

	/** `kitsuId`'s pending retry, while its trackers are still connected as
	 *  the users its save was made under: a retry made for one account is
	 *  not another's, nor a disconnected one's. Reactive to the account
	 *  store. */
	pendingFor(kitsuId: string): PendingEdit | null {
		const stored = this.pendings.get(kitsuId);
		if (!stored) return null;
		const holds = listingHolds(stored.listed, (provider, listed) =>
			followAccount(provider as Provider, listed)()
		);
		return holds ? stored.pending : null;
	}

	/** Set `kitsuId`'s pending retry, with the accounts the save that left it
	 *  was listed under; `null` clears it. */
	setPending(
		kitsuId: string,
		pending: PendingEdit | null,
		listed?: Readonly<Record<string, ListedAccount>>
	): void {
		this.pendings = withPending(this.pendings, kitsuId, pending ? { pending, listed } : null);
	}
}

export const editorFlights = new EditorFlightStore();
