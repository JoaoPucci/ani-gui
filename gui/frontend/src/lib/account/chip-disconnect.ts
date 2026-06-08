/**
 * Disconnect-flow helper for the topbar AccountChip.
 *
 * The /account page has its own copy of the same flow because it
 * couples with the per-card UI; this helper isolates the chip's
 * dispatch so the success / token-clear-failed branches get unit
 * coverage without mounting a Svelte component, per AGENTS.md §2.
 */

import type { DisconnectFlowDeps, DisconnectResult } from './connect-flow';
import type { Provider, ProviderState } from './types';

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
	pushToast(t: { kind: 'error'; message: string }): void;
	unknownErrorMessage(): string;
	tokenClearFailedMessage(): string;
}

export async function handleChipDisconnect(
	provider: Provider,
	prev: ProviderState,
	deps: ChipDisconnectDeps,
	cb: ChipDisconnectCallbacks
): Promise<void> {
	const r = await deps.disconnectAccount(provider, prev, {
		clearPersistedAccount: deps.clearPersistedAccount,
		dropListCache: deps.dropListCache,
		dropProviderCache: deps.dropProviderCache
	});
	if (r.kind === 'token_clear_failed') {
		cb.setError(provider, cb.unknownErrorMessage());
		cb.pushToast({ kind: 'error', message: cb.tokenClearFailedMessage() });
		return;
	}
	cb.setDisconnected(provider);
}
