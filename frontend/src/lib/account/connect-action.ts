/**
 * The /account page's Connect / Sign in again and Cancel, run against
 * the account store. Moved out of `+page.svelte` so the page stays a
 * thin adapter and every outcome is unit-tested (AGENTS.md §2).
 *
 * A sign-in always settles. Besides the outcomes the flow answers with
 * (`ConnectFlowResult`), a bridge call it makes can reject: the
 * main-process handlers catch their own failures, so a rejection is an
 * IPC failure, and the app cannot tell whether the request behind it
 * was carried out. The provider then goes back to where the click found
 * it at once — clearing `connecting`, which wakes a keyring read
 * waiting on it — and is read again from the disk, which knows whether
 * a sign-in was saved. The user is told the sign-in failed unless the
 * disk shows it landed.
 *
 * A cancel whose bridge call rejects still cancels: the sign-in it was
 * for is abandoned and the provider put back. Whatever the abandoned
 * sign-in does later is learned from the disk, never applied over a
 * newer state.
 */

import { m } from '$lib/paraglide/messages';
import { connectAccount, type ConnectFlowDeps, type ConnectFlowResult } from './connect-flow';
import { restoreAfterFailedConnect } from './state-helpers';
import { connectFailureMessage } from './connect-messages';
import type { PersistedAccount, Provider, ProviderState } from './types';

/** What a connect reads and writes on the account store. */
export interface ConnectActionStore {
	byProvider: Record<Provider, ProviderState>;
	setConnecting(provider: Provider): void;
	setConnected(provider: Provider, account: PersistedAccount): void;
	setDisconnected(provider: Provider): void;
	setExpired(provider: Provider, account: PersistedAccount): void;
	setError(provider: Provider, message: string): void;
	reread(provider: Provider): Promise<boolean>;
}

type ErrorToast = { kind: 'error'; message: string };

export interface ConnectActionDeps {
	store: ConnectActionStore;
	flow: ConnectFlowDeps;
	seedConnectedList(provider: Provider, bearer: string): Promise<void>;
	pushToast(toast: ErrorToast): void;
}

export interface CancelConnectDeps {
	store: ConnectActionStore;
	cancelOAuth(): Promise<boolean>;
	pushToast(toast: ErrorToast): void;
}

/** A sign-in under way: the state the click found, and whether a
 *  cancel that could not reach the main process gave up on it. */
interface Attempt {
	prev: ProviderState;
	abandoned: boolean;
}

const attempts = new Map<Provider, Attempt>();

/**
 * Sign in to `provider`, from whatever state it is in (signed out,
 * expired, error). Settles the provider whatever the flow does.
 */
export async function connectProvider(provider: Provider, deps: ConnectActionDeps): Promise<void> {
	const attempt: Attempt = { prev: deps.store.byProvider[provider], abandoned: false };
	deps.store.setConnecting(provider);
	attempts.set(provider, attempt);
	let r: ConnectFlowResult | null;
	try {
		r = await connectAccount(provider, deps.flow);
	} catch {
		r = null;
	}
	if (attempts.get(provider) === attempt) attempts.delete(provider);
	if (attempt.abandoned) {
		await deps.store.reread(provider);
		return;
	}
	if (r === null) return settleRejected(provider, attempt.prev, deps);
	if (r.kind === 'connected') {
		deps.store.setConnected(provider, r.account);
		// Seed the list cache the rails and stats read. Best-effort: the
		// sign-in stands either way, and the next rail mount warms it.
		await deps.seedConnectedList(provider, r.account.access_token);
		return;
	}
	restore(deps.store, provider, attempt.prev);
	settleFailed(provider, r, deps);
}

/**
 * Cancel the sign-in under way. The main process stops waiting for the
 * browser and the sign-in settles as cancelled. When that request
 * cannot be delivered, the sign-in is abandoned here instead.
 */
export async function cancelConnect(provider: Provider, deps: CancelConnectDeps): Promise<void> {
	try {
		await deps.cancelOAuth();
		return;
	} catch {
		/* not delivered: abandon the sign-in below */
	}
	const attempt = attempts.get(provider);
	if (!attempt) return;
	attempts.delete(provider);
	attempt.abandoned = true;
	restore(deps.store, provider, attempt.prev);
	deps.pushToast({ kind: 'error', message: m.account_connect_error_cancelled() });
}

/** A bridge call rejected: put the provider back, read the disk, and
 *  say the sign-in failed unless the disk shows it landed. */
async function settleRejected(
	provider: Provider,
	prev: ProviderState,
	deps: ConnectActionDeps
): Promise<void> {
	restore(deps.store, provider, prev);
	const read = await deps.store.reread(provider);
	if (read && deps.store.byProvider[provider].kind === 'connected') return;
	deps.pushToast({ kind: 'error', message: m.account_connect_error_unknown() });
}

/** Tell the user why a sign-in that answered did not succeed. */
function settleFailed(
	provider: Provider,
	r: Exclude<ConnectFlowResult, { kind: 'connected' }>,
	deps: ConnectActionDeps
): void {
	if (r.kind === 'persist_failed') {
		// A keychain that cannot encrypt is named, so a user without a
		// usable keyring learns what to install rather than to retry.
		const message =
			r.reason === 'encryption_unavailable'
				? m.account_persist_error_keychain_unavailable()
				: m.account_connect_error_unknown();
		deps.store.setError(provider, message);
		deps.pushToast({ kind: 'error', message });
		return;
	}
	deps.pushToast({ kind: 'error', message: connectFailureMessage(r) });
}

/**
 * Put the provider back where a failed sign-in found it (see
 * `restoreAfterFailedConnect`). Every branch goes through a store
 * setter, which clears the change flag and wakes whoever waits on it.
 */
function restore(store: ConnectActionStore, provider: Provider, prev: ProviderState): void {
	const restored = restoreAfterFailedConnect(prev);
	if (restored.kind === 'expired') {
		store.setExpired(provider, restored.account);
		return;
	}
	if (restored.kind === 'error') {
		// setError takes the account from the state it replaces: put the
		// sign-in back first so the error keeps it.
		if (restored.account) store.setExpired(provider, restored.account);
		store.setError(provider, restored.message);
		return;
	}
	store.setDisconnected(provider);
}
