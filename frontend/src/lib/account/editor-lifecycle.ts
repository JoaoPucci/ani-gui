// What the list editor decides about a write that outlives the instance
// that started it. The detail page unmounts the editor when it leaves a
// show — another route, or another show — so a save waiting out a
// tracker's rate limit can settle after its editor is gone, or after a
// new editor has mounted on the same show. The flights and pending
// retries live in a module store (`editor-flights.svelte.ts`); these
// pure rules say how each instance reads and settles them.

import type { Flight } from './editor-flight';
import type { ListedAccount } from './held-account';
import type { PendingEdit } from './list-entry-pending';

/**
 * Whether `flight` still locks its show's editor: some tracker it listed
 * is still connected as the user it was listed under (`stillFor`). Once
 * none is, its held writes are superseded anyway and the editor is free.
 * A flight with no listing locks.
 */
export function flightHolds(
	flight: Flight,
	stillFor: (provider: string, listed: ListedAccount) => boolean
): boolean {
	if (!flight.listed) return true;
	return Object.entries(flight.listed).some(([provider, listed]) => stillFor(provider, listed));
}

/** What an editor instance shows: its show, and the flight locking it. */
export interface Watched {
	kitsuId: string;
	flight: Flight | null;
}

/**
 * Whether a flight this instance did not start ended on the show it is
 * showing — between the two readings `prev` and `next` — so the instance
 * must re-read the entry itself: the instance that started it, and the
 * page it would have re-read, are gone.
 */
export function adoptedFlightLanded(prev: Watched, next: Watched, self: symbol): boolean {
	return (
		prev.kitsuId === next.kitsuId &&
		prev.flight !== null &&
		next.flight === null &&
		prev.flight.owner !== self
	);
}

/** Whether a write's continuation may touch its instance's own state:
 *  the instance is alive and still on the show the write was for. */
export function stillHere(
	instance: { destroyed: boolean; kitsuId: string },
	kitsuId: string
): boolean {
	return !instance.destroyed && instance.kitsuId === kitsuId;
}

export type Pendings = ReadonlyMap<string, PendingEdit>;

export const NO_PENDINGS: Pendings = new Map();

/** `pendings` with `kitsuId`'s pending retry set, or cleared for `null`. */
export function withPending(
	pendings: Pendings,
	kitsuId: string,
	pending: PendingEdit | null
): Pendings {
	if (pending === null && !pendings.has(kitsuId)) return pendings;
	const next = new Map(pendings);
	if (pending === null) next.delete(kitsuId);
	else next.set(kitsuId, pending);
	return next;
}
