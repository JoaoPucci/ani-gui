// The live wiring of `saveWhenAllowed` for a tracker write the user
// made: the notice that it waits, on the toast stack, naming the tracker
// by its display name, and a real timer. Split from the pure module so
// that one stays free of the toast store and the message bundle.

import type { Provider } from './types';
import { saveWhenAllowed, sleep, SupersededError } from './save-when-allowed';
import { toastStore } from '$lib/toasts/store.svelte';
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

/** The latest write issued per tracker and show. A write held back by a
 *  rate limit is sent again only while it is still the latest: a newer
 *  write for the same show — issued or landed during the wait — has
 *  said what the tracker should hold, and the held one would undo it. */
const generations = new Map<string, number>();

/** Run a user's write of `kitsuId` to `provider`, waiting out its rate
 *  limit with a notice — see `saveWhenAllowed`. Each wait replaces the
 *  write's earlier notice; a write a newer one superseded ends in
 *  `SupersededError`, unsent, and its notice is taken down. */
export function saveWhenTrackerAllows<T>(
	provider: Provider,
	kitsuId: string,
	attempt: () => Promise<T>
): Promise<T> {
	const key = `${provider}:${kitsuId}`;
	const generation = (generations.get(key) ?? 0) + 1;
	generations.set(key, generation);
	let notice: string | null = null;
	const takeDown = () => {
		if (notice !== null) toastStore.dismiss(notice);
		notice = null;
	};
	return saveWhenAllowed(attempt, {
		notify: (seconds) => {
			takeDown();
			notice = noticeBusy(provider, seconds);
		},
		wait: sleep,
		stillWanted: () => generations.get(key) === generation
	}).catch((e: unknown) => {
		if (e instanceof SupersededError) takeDown();
		throw e;
	});
}

/** The error toast for a progress write that could not be saved. */
export function reportProgressNotSaved(provider: Provider): void {
	toastStore.push({
		kind: 'error',
		message: m.account_progress_save_failed({ provider: providerName(provider) })
	});
}
