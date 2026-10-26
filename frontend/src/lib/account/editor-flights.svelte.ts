// The list editor's writes in flight and pending retries, kept for the
// life of the app rather than of an editor instance: the detail page
// unmounts its editor whenever it leaves a show, while a save can wait
// out a tracker's rate limit for minutes. The rules are pure, in
// `editor-flight.ts` and `editor-lifecycle.ts`; this holds the state.

import type { Provider } from './types';
import { accountStanding } from './held-account';
import {
	flightFor,
	NO_FLIGHTS,
	withFlight,
	withoutFlight,
	type Flight,
	type Flights
} from './editor-flight';
import { flightHolds, listingHolds } from './editor-lifecycle';
import {
	heldPending,
	NO_SHOW_PENDINGS,
	settlePending,
	type Listing,
	type ShowPendings
} from './editor-pendings';
import type { PendingEdit } from './list-entry-pending';

class EditorFlightStore {
	flights = $state.raw<Flights>(NO_FLIGHTS);
	pendings = $state.raw<ShowPendings>(NO_SHOW_PENDINGS);

	/** The write in flight that still locks `kitsuId`'s editor: one whose
	 *  trackers are still connected as the users it listed. Reactive to
	 *  the account store. */
	locking(kitsuId: string): Flight | null {
		const flight = flightFor(this.flights, kitsuId);
		if (!flight) return null;
		const holds = flightHolds(flight, (provider, listed) =>
			accountStanding(provider as Provider, listed)
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

	/** `kitsuId`'s pending retry, while the accounts its save was made
	 *  under still hold (`listingHolds`): some tracker it listed is still
	 *  connected as that user and none is another user's. While they do
	 *  not, the retry is hidden, not cleared — it is that account's intent,
	 *  and comes back with it. Reactive to the account store. */
	pendingFor(kitsuId: string): PendingEdit | null {
		return heldPending(this.pendings, kitsuId, (listed) =>
			listingHolds(listed, (provider, account) => accountStanding(provider as Provider, account))
		);
	}

	/** A write under `listed` settled for `kitsuId`: its own listing's retry
	 *  becomes `next(that retry)`; no other listing's is touched. */
	settlePending(
		kitsuId: string,
		listed: Listing | undefined,
		next: (prev: PendingEdit | null) => PendingEdit | null
	): void {
		this.pendings = settlePending(this.pendings, kitsuId, listed, next);
	}
}

export const editorFlights = new EditorFlightStore();
