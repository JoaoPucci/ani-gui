// A show's pending partial-save retries, each with its listing — the
// accounts the save that left it was made under. A retry is that
// listing's intent: a save or removal settles, replaces or clears every
// retry whose listing is compatible with its own (`listingsCompatible`),
// and no other, and the editor is offered the newest retry whose
// listing still holds. Pure: the app-lifetime store
// `editor-flights.svelte.ts` holds the map.

import type { ListedAccount } from './held-account';
import type { PendingEdit } from './list-entry-pending';

/** The accounts a write's fan-out listed its trackers under. */
export type Listing = Readonly<Record<string, ListedAccount>>;

/** One listing's pending retry for a show. */
export interface ListedPending {
	pending: PendingEdit;
	listed?: Listing;
}

/** Per show, its retries, oldest first. */
export type ShowPendings = ReadonlyMap<string, readonly ListedPending[]>;

export const NO_SHOW_PENDINGS: ShowPendings = new Map();

/** A listing's identity: its trackers and users, in a fixed order. A
 *  write with no listing has an identity of its own. */
export function listingKey(listed?: Listing): string {
	if (!listed) return '*';
	return Object.keys(listed)
		.sort()
		.map((provider) => `${provider}=${listed[provider].user ?? ''}`)
		.join('&');
}

/**
 * Whether a write under `a` settles a retry left under `b`: the same
 * listing, or one a tracker connected or disconnected since made larger
 * or smaller with no account switched — no tracker the two share is
 * listed under different users, and at least one they share is listed
 * under the same, known user. Symmetric.
 */
export function listingsCompatible(a?: Listing, b?: Listing): boolean {
	if (listingKey(a) === listingKey(b)) return true;
	if (a === undefined || b === undefined) return false;
	const shared = Object.keys(a).filter((provider) => provider in b);
	const switched = shared.some((provider) => a[provider].user !== b[provider].user);
	const kept = shared.some(
		(provider) => a[provider].user != null && a[provider].user === b[provider].user
	);
	return !switched && kept;
}

/** The newest retry for `kitsuId` a write under `listed` settles,
 *  whether or not it holds. */
export function pendingUnder(
	pendings: ShowPendings,
	kitsuId: string,
	listed?: Listing
): PendingEdit | null {
	const entries = pendings.get(kitsuId) ?? [];
	for (let i = entries.length - 1; i >= 0; i -= 1) {
		if (listingsCompatible(listed, entries[i].listed)) return entries[i].pending;
	}
	return null;
}

/**
 * `pendings` once a write under `listed` settled for `kitsuId`: every
 * retry compatible with `listed` goes, `next(the newest of them)` — when
 * not `null` — becomes the show's newest, and every other retry is left
 * as it was.
 */
export function settlePending(
	pendings: ShowPendings,
	kitsuId: string,
	listed: Listing | undefined,
	next: (prev: PendingEdit | null) => PendingEdit | null
): ShowPendings {
	const prev = pendingUnder(pendings, kitsuId, listed);
	const pending = next(prev);
	if (pending === prev) return pendings;
	const others = (pendings.get(kitsuId) ?? []).filter((p) => !listingsCompatible(listed, p.listed));
	const entries = pending === null ? others : [...others, { pending, listed }];
	const map = new Map(pendings);
	if (entries.length === 0) map.delete(kitsuId);
	else map.set(kitsuId, entries);
	return map;
}

/** The newest of `kitsuId`'s retries whose listing still `holds`. */
export function heldPending(
	pendings: ShowPendings,
	kitsuId: string,
	holds: (listed?: Listing) => boolean
): PendingEdit | null {
	const entries = pendings.get(kitsuId) ?? [];
	for (let i = entries.length - 1; i >= 0; i -= 1) {
		if (holds(entries[i].listed)) return entries[i].pending;
	}
	return null;
}
