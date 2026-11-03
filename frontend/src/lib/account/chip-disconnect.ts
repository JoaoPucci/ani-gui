/**
 * The disconnect of both the topbar AccountChip and the /account page,
 * kept here so its outcomes are unit-tested without mounting a Svelte
 * component (AGENTS.md §2).
 *
 * A disconnect always settles. Besides the clear failing with an
 * answer (`token_clear_failed`), the bridge call behind it can reject:
 * the main-process handler catches its own failures, so a rejection is
 * an IPC failure, and the app cannot tell whether the token was
 * cleared. The provider then leaves its account change at once, in the
 * error state a failed clear sets (which wakes a keyring read waiting
 * on the change), and is read again from the disk, which knows. The
 * user is told the clear failed unless the disk shows it landed; a
 * keyring slow to answer does not hold that notice back
 * (`landed-notice`).
 */

import type { DisconnectFlowDeps, DisconnectResult } from './connect-flow';
import type { Provider, ProviderState } from './types';
import { KEYRING_WAIT_MS } from './keyring-wait';
import { noticeUnlessLanded, realTimers } from './landed-notice';

export interface ChipDisconnectDeps extends DisconnectFlowDeps {
	disconnectAccount: (
		provider: Provider,
		prev: ProviderState,
		ops: DisconnectFlowDeps
	) => Promise<DisconnectResult>;
}

export interface ChipDisconnectCallbacks {
	setError(provider: Provider, message: string): void;
	setDisconnected(provider: Provider): void;
	/** Show a notice; returns its id. */
	pushToast(t: { kind: 'error'; message: string }): string;
	dismissToast(id: string): void;
	unknownErrorMessage(): string;
	tokenClearFailedMessage(): string;
	/** Read the provider's saved sign-in again and apply it; true when
	 *  applied (`accountStore.reread`). */
	reread(provider: Provider): Promise<boolean>;
	stateOf(provider: Provider): ProviderState;
}

export async function handleChipDisconnect(
	provider: Provider,
	prev: ProviderState,
	deps: ChipDisconnectDeps,
	cb: ChipDisconnectCallbacks
): Promise<void> {
	let r: DisconnectResult;
	try {
		r = await deps.disconnectAccount(provider, prev, {
			beginAccountChange: deps.beginAccountChange,
			clearPersistedAccount: deps.clearPersistedAccount,
			dropListCache: deps.dropListCache,
			dropProviderCache: deps.dropProviderCache
		});
	} catch {
		return settleRejected(provider, cb);
	}
	if (r.kind === 'token_clear_failed') {
		cb.setError(provider, cb.unknownErrorMessage());
		cb.pushToast({ kind: 'error', message: cb.tokenClearFailedMessage() });
		return;
	}
	cb.setDisconnected(provider);
}

/** The clear's bridge call rejected: leave the change in the error
 *  state, read the disk, and say the clear failed unless it landed. */
async function settleRejected(provider: Provider, cb: ChipDisconnectCallbacks): Promise<void> {
	cb.setError(provider, cb.unknownErrorMessage());
	await noticeUnlessLanded({
		reread: () => cb.reread(provider),
		landed: () => cb.stateOf(provider).kind === 'disconnected',
		push: () => cb.pushToast({ kind: 'error', message: cb.tokenClearFailedMessage() }),
		dismiss: cb.dismissToast,
		waitMs: KEYRING_WAIT_MS,
		...realTimers
	});
}
