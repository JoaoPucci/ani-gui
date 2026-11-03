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
 * the user did since is newer than what it read. Once that change has
 * landed, a provider it left signed out is read again (see
 * `readAfterChange`). A provider whose read
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
	refreshExpired(only?: Provider): Promise<void>;
}

const PROVIDERS: Provider[] = ['anilist', 'mal', 'inhouse'];

/** One hydrate: whether its first reads have all settled, and its
 *  refresh run. */
interface HydrateRun {
	settled: boolean;
}

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

/** The provider's state kind, read now. */
function kindOf(target: HydrateTarget, p: Provider): ProviderState['kind'] {
	return target.byProvider[p].kind;
}

/** Read the provider once; true when the answer was applied, false
 *  when a change overtook it. */
async function readOnce(target: HydrateTarget, p: Provider, wait: KeyringWait): Promise<boolean> {
	const generation = target.accountGeneration[p];
	const reading = readPersistedAccount(p);
	wait.track(reading);
	const read = await reading;
	if (target.accountGeneration[p] !== generation) return false;
	applyRead(target, p, read);
	return true;
}

/**
 * Read one provider again and apply the answer, outside any hydrate:
 * after an account action whose bridge call rejected, the disk is the
 * only one who knows whether its write or clear landed. A change under
 * way is left alone (it settles the provider itself), and a read a
 * change overtakes is dropped, as in the hydrate. True when the answer
 * was applied.
 */
export function rereadAccount(target: HydrateTarget, p: Provider): Promise<boolean> {
	if (changing(target, p)) return Promise.resolve(false);
	return readOnce(target, p, waitFor(target));
}

/**
 * After a change got in a read's way: wait until it has landed — read
 * while a connect or disconnect is under way, an answer would replace
 * the change's own state — and then read again only if it left the
 * provider signed out. That is a connect started and cancelled, where
 * the disk still holds the sign-in the overtaken read was after. A
 * change that ended anywhere else (signed in, an error it reported) has
 * said what the provider's state is. A sign-in found expired after the
 * hydrate's own refresh has run gets one of its own, for that provider.
 */
async function readAfterChange(
	target: HydrateTarget,
	p: Provider,
	wait: KeyringWait,
	hydrate: HydrateRun
): Promise<void> {
	for (;;) {
		while (changing(target, p)) await target.nextChange(p);
		if (kindOf(target, p) !== 'disconnected') return;
		if (await readOnce(target, p, wait)) {
			// While the hydrate is still out its own refresh covers this
			// sign-in; asking here too would exchange one refresh token twice.
			if (hydrate.settled && kindOf(target, p) === 'expired') void target.refreshExpired(p);
			return;
		}
	}
}

/**
 * The provider's first read. Settles once it has applied, or once a
 * change has got in its way — then the rest goes on in the background,
 * so a sign-in left open in the browser holds no other provider's
 * hydrate or refresh. The keyring notice counts the reads alone, not
 * the waits for a change.
 */
async function firstRead(
	target: HydrateTarget,
	p: Provider,
	wait: KeyringWait,
	hydrate: HydrateRun
): Promise<void> {
	if (!changing(target, p) && (await readOnce(target, p, wait))) return;
	void readAfterChange(target, p, wait, hydrate);
}

/**
 * Read every provider and apply each answer — a provider a change got in
 * the way of is read in the background (see `firstRead`) — then give a
 * just-expired but refreshable provider its refresh. Every hydrate path (cold launch
 * AND the account page) must do that, so the hydrate owns it rather
 * than each caller remembering.
 */
export function hydrateAccounts(target: HydrateTarget): Promise<void> {
	const wait = waitFor(target);
	const hydrate: HydrateRun = { settled: false };
	const settled = Promise.all(PROVIDERS.map((p) => firstRead(target, p, wait, hydrate)));
	return settled.then(() => {
		hydrate.settled = true;
		return target.refreshExpired();
	});
}
