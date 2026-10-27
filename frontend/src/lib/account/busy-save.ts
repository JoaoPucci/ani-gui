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
import { coverFields, fieldsLeft, type Coverage } from './write-fields';
import { startCountdown } from './busy-countdown';
import { toastStore } from '$lib/toasts/store.svelte';
import { followAccount, type ListedAccount } from './held-account';
import { m } from '$lib/paraglide/messages';

/** A tracker's display name; the provider id for one without a name. */
export function providerName(provider: Provider): string {
	if (provider === 'anilist') return m.account_provider_anilist();
	if (provider === 'mal') return m.account_provider_mal();
	return provider;
}

/** How long the busy notice stays up for a wait of `seconds`: the wait
 *  itself, and no shorter than a plain toast. */
export function noticeDurationMs(seconds: number): number {
	return Math.max(4000, seconds * 1000);
}

/** The notice's words with `seconds` left: the wait, or once it is over,
 *  that the save is going. */
function busyText(provider: Provider, seconds: number): string {
	const name = providerName(provider);
	return seconds > 0
		? m.account_busy_saving({ provider: name, seconds })
		: m.account_busy_saving_now({ provider: name });
}

/** A real once-a-second timer, wrapped in arrows: `setInterval` called
 *  off an object is an "Illegal invocation" in Chromium. */
function everySecond(ms: number, tick: () => void): () => void {
	const handle = setInterval(() => tick(), ms);
	return () => clearInterval(handle);
}

/** The toast shown while `provider` waits `seconds` to take a write. It
 *  counts the wait down in place, read from one deadline, and then says
 *  the save is going; it stays up until taken down — by the write it is
 *  about, a newer write, or the user — and its timer stops whenever it
 *  leaves. Returns its id. */
export function noticeBusy(provider: Provider, seconds: number): string {
	let stop = () => {};
	const id = toastStore.push({
		kind: 'info',
		message: busyText(provider, seconds),
		duration: null,
		onClose: () => stop()
	});
	stop = startCountdown(Date.now() + seconds * 1000, {
		now: () => Date.now(),
		every: everySecond,
		show: (left) => toastStore.update(id, busyText(provider, left))
	});
	return id;
}

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
		notify: (seconds) => {
			takeDownNotice(key);
			mine = noticeBusy(provider, seconds);
			notices.set(key, mine);
		},
		wait: sleep,
		stillWanted: () => held.left().length > 0
	}).finally(() => {
		// Landed, failed or superseded: its notice goes with it, unless a
		// newer write's has replaced it.
		if (mine !== null && notices.get(key) === mine) takeDownNotice(key);
		settle(key);
	});
}

/** The error toast for a progress write that could not be saved. */
export function reportProgressNotSaved(provider: Provider): void {
	toastStore.push({
		kind: 'error',
		message: m.account_progress_save_failed({ provider: providerName(provider) })
	});
}
