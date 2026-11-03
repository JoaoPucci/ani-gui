/**
 * One provider's saved sign-in read from the OS keyring and applied to
 * the account store: the read the hydrate makes for each provider
 * (`$lib/account/hydrate-flow`), and the one an account action makes
 * after its bridge call rejected (`rereadAccount`). Split from the
 * hydrate so each file stays inside the CRAP gate's per-file bar.
 */

import type { Provider, ProviderState } from './types';
import type { ReadPersistedAccountResult } from './bridge';
import type { RenewalFailures } from './renewal-failure';
import { readPersistedAccount } from './api';
import { ownWrites } from './own-writes';
import { failuresAfterHydrate, hydratedState, isSignInChange } from './hydrated-state';
import { KeyringWait } from './keyring-wait';

/** What a read and writes on the account store. */
export interface HydrateTarget {
	byProvider: Record<Provider, ProviderState>;
	renewalFailure: RenewalFailures;
	accountGeneration: Record<Provider, number>;
	accountChanging: Record<Provider, boolean>;
	keyringWaiting: boolean;
	bumpGeneration(provider: Provider): void;
	nextChange(provider: Provider): Promise<void>;
	refreshExpired(only?: Provider): Promise<void>;
}

/** One KeyringWait per store, shared by its overlapping hydrates. */
const waits = new WeakMap<HydrateTarget, KeyringWait>();

export function waitFor(target: HydrateTarget): KeyringWait {
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
export function changing(target: HydrateTarget, p: Provider): boolean {
	return target.accountChanging[p] || target.byProvider[p].kind === 'connecting';
}

/** Read the provider once; true when the answer was applied, false
 *  when a change overtook it or `accept` turned the answer down. */
export async function readOnce(
	target: HydrateTarget,
	p: Provider,
	wait: KeyringWait,
	accept: (read: ReadPersistedAccountResult) => boolean = () => true
): Promise<boolean> {
	const generation = target.accountGeneration[p];
	const reading = readPersistedAccount(p);
	wait.track(reading);
	const read = await reading;
	if (target.accountGeneration[p] !== generation || !accept(read)) return false;
	applyRead(target, p, read);
	return true;
}

/** Whether a read says what the disk holds: a sign-in, or none. A read
 *  the keyring could not answer says neither. */
function readTheDisk(read: ReadPersistedAccountResult): boolean {
	return read.ok || read.kind === 'not_found';
}

/**
 * Read one provider again and apply the answer, outside any hydrate:
 * after an account action whose bridge call rejected, the disk is the
 * only one who knows whether its write or clear landed. A change under
 * way is left alone (it settles the provider itself), and a read a
 * change overtakes is dropped, as in the hydrate. So is a read the
 * keyring could not answer: unlike the hydrate's, this one replaces a
 * state the action restored, and a failed read says nothing about what
 * the action did. True when the answer was applied.
 */
export function rereadAccount(target: HydrateTarget, p: Provider): Promise<boolean> {
	if (changing(target, p)) return Promise.resolve(false);
	return readOnce(target, p, waitFor(target), readTheDisk);
}
