// The live wiring of `saveWhenAllowed` for a tracker write the user
// made: the ledger of writes per tracker and show that decides what a
// held write may still send, a real timer, and the busy notice
// (`busy-notice.ts`) put up and taken down around each wait. Split from
// the pure module so that one stays free of the toast store.

import type { Provider } from './types';
import {
	ALL_FIELDS,
	saveWhenAllowed,
	waitWhileWanted,
	SupersededError,
	type HeldWrite,
	type WriteField
} from './save-when-allowed';
import { coverFields, fieldsLeft, type Coverage } from './write-fields';
import { toastStore } from '$lib/toasts/store.svelte';
import { accountLeft, followAccount, type ListedAccount } from './held-account';
import { after, monotonic, noticeBusy } from './busy-notice';

/** Per tracker and show: the count of writes started, the newest write
 *  to cover each field, and the busy notice on screen. A write held
 *  back by a rate limit sends again only the fields no newer write for
 *  the same show has covered — a newer write, issued or landed during
 *  the wait, has said what the tracker should hold for those, and the
 *  held one would undo it. Nothing left, and it is superseded. */
const started = new Map<string, number>();
const inFlight = new Map<string, number>();
const coverage = new Map<string, Coverage>();
const notices = new Map<string, string>();

/** How many tracker-and-show keys the ledger holds. */
export function trackedWriteKeys(): number {
	return started.size;
}

function takeDownNotice(key: string): void {
	const id = notices.get(key);
	if (id !== undefined) toastStore.dismiss(id);
	notices.delete(key);
}

/** Record a write of `fields` starting for `key`; returns its number. */
function start(key: string, fields: readonly WriteField[]): number {
	const generation = (started.get(key) ?? 0) + 1;
	started.set(key, generation);
	inFlight.set(key, (inFlight.get(key) ?? 0) + 1);
	coverage.set(key, coverFields(coverage.get(key), fields, generation));
	// The older write's notice says something this write may change.
	takeDownNotice(key);
	return generation;
}

/** A write for `key` settled. With none left in flight, nothing about
 *  the key can matter to a later write, and the ledger forgets it. */
function settle(key: string): void {
	const left = (inFlight.get(key) ?? 1) - 1;
	if (left > 0) {
		inFlight.set(key, left);
		return;
	}
	inFlight.delete(key);
	started.delete(key);
	coverage.delete(key);
}

/** Run a user's write of `kitsuId` to `provider`, waiting out its rate
 *  limit with a notice — see `saveWhenAllowed`. The attempt asks its
 *  `HeldWrite` which of `fields` are still its own, and sends only
 *  those; a write with none left ends in `SupersededError`, unsent. Each
 *  wait replaces the show's notice, and a newer write takes it down. */
export function saveWhenTrackerAllows<T>(
	provider: Provider,
	kitsuId: string,
	attempt: (held: HeldWrite) => Promise<T>,
	fields: readonly WriteField[] = ALL_FIELDS,
	listed?: ListedAccount
): Promise<T> {
	const key = `${provider}:${kitsuId}`;
	// Started even when it will not go: a write dropped for its account
	// still counts as the newer one for the fields it set.
	const generation = start(key, fields);
	// The tracker account the write was made for. Changed since — or
	// being changed as it was made — nothing of it is left, and it goes
	// no further: not its re-read, not its write.
	const sameAccount = followAccount(provider, listed);
	const held: HeldWrite = {
		left: () => (sameAccount() ? fieldsLeft(coverage.get(key), fields, generation) : [])
	};
	const run = () =>
		held.left().length > 0 ? attempt(held) : Promise.reject(new SupersededError());
	// This write's notice, while it is the show's.
	let mine: string | null = null;
	return saveWhenAllowed(run, {
		notify: (seconds, refusedBy) => {
			takeDownNotice(key);
			mine = noticeBusy(provider, seconds, refusedBy);
			notices.set(key, mine);
		},
		// Ended early only on what cannot come back: newer writes took every
		// field, or the tracker is disconnected or another user's. A sign-in
		// or an account change under way may end on the same user; the
		// checks at the end of the wait and before the send decide then.
		wait: (ms) =>
			waitWhileWanted(ms, {
				after,
				now: monotonic,
				stillWanted: () =>
					fieldsLeft(coverage.get(key), fields, generation).length > 0 &&
					!accountLeft(provider, listed)
			}),
		stillWanted: () => held.left().length > 0
	}).finally(() => {
		// Landed, failed or superseded: its notice goes with it, unless a
		// newer write's has replaced it.
		if (mine !== null && notices.get(key) === mine) takeDownNotice(key);
		settle(key);
	});
}

export { noticeBusy, providerName, reportProgressNotSaved } from './busy-notice';
