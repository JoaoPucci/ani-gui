/**
 * One sign-in under way, as `connect-action` runs it: what it restores
 * when it fails, and the save it holds back once abandoned. Split from
 * `connect-action` so each file stays inside the CRAP gate's per-file
 * bar.
 */

import type { ConnectFlowDeps } from './connect-flow';
import type { ConnectActionStore } from './connect-action';
import { restoreAfterFailedConnect } from './state-helpers';
import type { Provider, ProviderState } from './types';

/** A sign-in under way: the state the click found, whether a cancel
 *  that could not reach the main process gave up on it, and whether it
 *  sent its sign-in to be saved. */
export interface Attempt {
	prev: ProviderState;
	abandoned: boolean;
	saveSent: boolean;
}

/**
 * The flow's save, held back once the sign-in is abandoned: the user
 * was told it was cancelled. A save sent before that is noted, since it
 * may land.
 */
export function guarded(flow: ConnectFlowDeps, attempt: Attempt): ConnectFlowDeps {
	return {
		...flow,
		persistAccount: (provider, payload) => {
			if (attempt.abandoned) return Promise.resolve({ ok: false, kind: 'superseded' });
			attempt.saveSent = true;
			return flow.persistAccount(provider, payload);
		}
	};
}

/**
 * Put the provider back where a failed sign-in found it (see
 * `restoreAfterFailedConnect`). Every branch goes through a store
 * setter, which clears the change flag and wakes whoever waits on it.
 */
export function restore(store: ConnectActionStore, provider: Provider, prev: ProviderState): void {
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
