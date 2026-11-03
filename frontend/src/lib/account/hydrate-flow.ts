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
 * the user did since is newer than what it read; the provider is read
 * again, so it ends on what that change left on disk. A provider whose read
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
	accountChanging: Record<Provider, boolean>;
	keyringWaiting: boolean;
	bumpGeneration(provider: Provider): void;
	nextChange(provider: Provider): Promise<void>;
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

/** Seed one provider from its keychain read. */
function applyRead(target: HydrateTarget, p: Provider, read: ReadPersistedAccountResult): void {
	const prev = target.byProvider;
	const state = hydratedState(prev[p], read, Math.floor(Date.now() / 1000));
	if (isSignInChange(prev[p], state, ownWrites(p))) target.bumpGeneration(p);
	const next = { ...prev, [p]: state };
	target.renewalFailure = failuresAfterHydrate(prev, next, target.renewalFailure);
	target.byProvider = next;
}

/** Whether a change to the provider is under way: a connect in the
 *  browser, or a disconnect or other account change not yet landed. */
function changing(target: HydrateTarget, p: Provider): boolean {
	return target.accountChanging[p] || target.byProvider[p].kind === 'connecting';
}

/**
 * Read one provider until a read answers with its generation where it
 * found it. An overtaken answer is dropped and the provider read again:
 * the change that overtook it may since have been undone (a connect
 * started and cancelled), and the disk says what it left. Every read,
 * the first included, waits until no change is under way — read while a
 * connect or disconnect is, its answer would replace the change's own
 * state. The keyring notice counts the reads alone, not that wait.
 */
async function readProvider(target: HydrateTarget, p: Provider, wait: KeyringWait): Promise<void> {
	for (;;) {
		while (changing(target, p)) await target.nextChange(p);
		const generation = target.accountGeneration[p];
		const reading = readPersistedAccount(p);
		wait.track(reading);
		const read = await reading;
		if (target.accountGeneration[p] === generation) return applyRead(target, p, read);
	}
}

/**
 * Read every provider and apply each answer; then give a just-expired
 * but refreshable provider its refresh. Every hydrate path (cold launch
 * AND the account page) must do that, so the hydrate owns it rather
 * than each caller remembering.
 */
export function hydrateAccounts(target: HydrateTarget): Promise<void> {
	const wait = waitFor(target);
	const settled = Promise.all(PROVIDERS.map((p) => readProvider(target, p, wait)));
	return settled.then(() => target.refreshExpired());
}
