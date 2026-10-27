// What the list editor decides about a write that outlives the instance
// that started it. The detail page unmounts the editor when it leaves a
// show — another route, or another show — so a save waiting out a
// tracker's rate limit can settle after its editor is gone, or after a
// new editor has mounted on the same show. The flights and pending
// retries live in a module store (`editor-flights.svelte.ts`); these
// pure rules say how each instance reads and settles them.

import type { Flight } from './editor-flight';
import type { ListedAccount } from './held-account';

/** Where a listed tracker's account stands against the user it was
 *  listed under — see `accountStanding` in `held-account.ts`. */
export type Standing = 'same' | 'other' | 'gone';

/**
 * Whether `flight` still locks its show's editor: some tracker it listed
 * is still connected as the user it was listed under — a held write to
 * it is still out. Once none is, its held writes are superseded anyway
 * and the editor is free. A flight with no listing locks.
 */
export function flightHolds(
	flight: Flight,
	standing: (provider: string, listed: ListedAccount) => Standing
): boolean {
	if (!flight.listed) return true;
	return Object.entries(flight.listed).some(([p, account]) => standing(p, account) === 'same');
}

/** Whether a pending retry's `listed` accounts still hold: some tracker
 *  is still connected as the user it was listed under, and none is
 *  another user's — a retry is the intent of the accounts it was made
 *  under, not of whoever holds one of them now. No listing holds. */
export function listingHolds(
	listed: Readonly<Record<string, ListedAccount>> | undefined,
	standing: (provider: string, listed: ListedAccount) => Standing
): boolean {
	if (!listed) return true;
	const now = Object.entries(listed).map(([p, account]) => standing(p, account));
	return now.includes('same') && !now.includes('other');
}

/** What an editor instance watches: its show, and the flight in flight
 *  for it — still locking or not. */
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

/** What an instance does when a flight another instance started ends on
 *  its show: re-read the entry, and close the popover when it is open on
 *  a flight's sent form (`shown`) — the re-read entry replaces it. */
export function adoptedLanding(
	prev: Watched,
	next: Watched,
	self: symbol,
	shown: Flight | null
): { reconcile: boolean; close: boolean } {
	const landed = adoptedFlightLanded(prev, next, self);
	return { reconcile: landed, close: landed && shown !== null && shown === prev.flight };
}

/** Whether a write's continuation may touch its instance's own state:
 *  the instance is alive and still on the show the write was for. */
export function stillHere(
	instance: { destroyed: boolean; kitsuId: string },
	kitsuId: string
): boolean {
	return !instance.destroyed && instance.kitsuId === kitsuId;
}
