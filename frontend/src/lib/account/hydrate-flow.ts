/**
 * `accountStore.hydrate()`'s reads: each provider's saved sign-in read
 * from the OS keyring and applied to the store as it answers. Split
 * from the store so the store stays inside the CRAP gate's per-file
 * bar.
 *
 * Asynchronous, provider by provider: a keyring that is locked or not
 * answering holds only its own reads, and the app renders signed out
 * until each answers. A read that answers after the provider's state
 * changed (a connect or disconnect while it waited) is dropped: what
 * the user did since is newer than what it read. A provider whose read
 * holds a different sign-in moves its generation, so a renewal still
 * out for the old one is superseded.
 */

import type { Provider, ProviderState } from './types';
import type { ReadPersistedAccountResult } from './bridge';
import type { RenewalFailures } from './renewal-failure';
import { readPersistedAccount } from './api';
import { ownWrites } from './own-writes';
import { failuresAfterHydrate, hydratedState, isSignInChange } from './hydrated-state';
import { KeyringWait } from './keyring-wait';

/** What a hydrate reads and writes on the account store. */
export interface HydrateTarget {
	byProvider: Record<Provider, ProviderState>;
	renewalFailure: RenewalFailures;
	accountGeneration: Record<Provider, number>;
	keyringWaiting: boolean;
	bumpGeneration(provider: Provider): void;
	refreshExpired(): Promise<void>;
}

const PROVIDERS: Provider[] = ['anilist', 'mal', 'inhouse'];

/** One KeyringWait per store, shared by its overlapping hydrates. */
const waits = new WeakMap<HydrateTarget, KeyringWait>();

function waitFor(target: HydrateTarget): KeyringWait {
	let wait = waits.get(target);
	if (!wait) {
		wait = new KeyringWait(
			{
				setTimer: (fn, ms) => setTimeout(() => fn(), ms),
				clearTimer: (h) => clearTimeout(h as ReturnType<typeof setTimeout>)
			},
			(waiting) => {
				target.keyringWaiting = waiting;
			}
		);
		waits.set(target, wait);
	}
	return wait;
}

/** Seed one provider from its keychain read, unless superseded. */
function applyRead(
	target: HydrateTarget,
	p: Provider,
	generation: number,
	read: ReadPersistedAccountResult
): void {
	if (target.accountGeneration[p] !== generation) return;
	const prev = target.byProvider;
	const state = hydratedState(prev[p], read, Math.floor(Date.now() / 1000));
	if (isSignInChange(prev[p], state, ownWrites(p))) target.bumpGeneration(p);
	const next = { ...prev, [p]: state };
	target.renewalFailure = failuresAfterHydrate(prev, next, target.renewalFailure);
	target.byProvider = next;
}

/**
 * Read every provider and apply each answer; then give a just-expired
 * but refreshable provider its refresh. Every hydrate path (cold launch
 * AND the account page) must do that, so the hydrate owns it rather
 * than each caller remembering.
 */
export function hydrateAccounts(target: HydrateTarget): Promise<void> {
	const reads = PROVIDERS.map((p) => {
		const generation = target.accountGeneration[p];
		return readPersistedAccount(p).then((read) => applyRead(target, p, generation, read));
	});
	const settled = Promise.all(reads);
	waitFor(target).track(settled);
	return settled.then(() => target.refreshExpired());
}
