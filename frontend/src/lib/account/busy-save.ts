// The live wiring of `saveWhenAllowed` for a tracker write the user
// made: the notice that it waits, on the toast stack, naming the tracker
// by its display name, and a real timer. Split from the pure module so
// that one stays free of the toast store and the message bundle.

import type { Provider } from './types';
import {
	ALL_FIELDS,
	saveWhenAllowed,
	sleep,
	SupersededError,
	type HeldWrite,
	type WriteField
} from './save-when-allowed';
import { toastStore } from '$lib/toasts/store.svelte';
import { followAccount, type ListedAccount } from './held-account';
import { m } from '$lib/paraglide/messages';

/** A tracker's display name; the provider id for one without a name. */
export function providerName(provider: Provider): string {
	if (provider === 'anilist') return m.account_provider_anilist();
	if (provider === 'mal') return m.account_provider_mal();
	return provider;
}

/** The toast shown while `provider` waits `seconds` to take a write: up
 *  for as long as the wait, and no shorter than the stack's default.
 *  Returns its id. */
export function noticeBusy(provider: Provider, seconds: number): string {
	return toastStore.push({
		kind: 'info',
		message: m.account_busy_saving({ provider: providerName(provider), seconds }),
		duration: Math.max(4000, seconds * 1000)
	});
}

/** Per tracker and show: the count of writes started, the newest write
 *  to cover each field, and the busy notice on screen. A write held
 *  back by a rate limit sends again only the fields no newer write for
 *  the same show has covered — a newer write, issued or landed during
 *  the wait, has said what the tracker should hold for those, and the
 *  held one would undo it. Nothing left, and it is superseded. */
const started = new Map<string, number>();
const inFlight = new Map<string, number>();
const coverage = new Map<string, Partial<Record<WriteField, number>>>();
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
	const covered = { ...coverage.get(key) };
	for (const field of fields) covered[field] = generation;
	coverage.set(key, covered);
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
		left: () =>
			sameAccount() ? fields.filter((field) => coverage.get(key)?.[field] === generation) : []
	};
	const run = () =>
		held.left().length > 0 ? attempt(held) : Promise.reject(new SupersededError());
	return saveWhenAllowed(run, {
		notify: (seconds) => {
			takeDownNotice(key);
			notices.set(key, noticeBusy(provider, seconds));
		},
		wait: sleep,
		stillWanted: () => held.left().length > 0
	})
		.catch((e: unknown) => {
			if (e instanceof SupersededError && started.get(key) === generation) takeDownNotice(key);
			throw e;
		})
		.finally(() => settle(key));
}

/** The error toast for a progress write that could not be saved. */
export function reportProgressNotSaved(provider: Provider): void {
	toastStore.push({
		kind: 'error',
		message: m.account_progress_save_failed({ provider: providerName(provider) })
	});
}
