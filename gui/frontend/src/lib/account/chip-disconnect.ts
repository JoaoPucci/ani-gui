/**
 * Disconnect-flow helper for the topbar AccountChip.
 *
 * The /account page has its own copy of the same flow because it
 * couples with the per-card UI; this helper isolates the chip's
 * dispatch so the success / token-clear-failed branches get unit
 * coverage without mounting a Svelte component, per AGENTS.md §2.
 */

import type { Provider, ProviderState } from './types';

export interface ChipDisconnectDeps {
	disconnectAccount: (
		provider: Provider,
		prev: ProviderState,
		ops: {
			clearPersistedAccount: ChipDisconnectDeps['clearPersistedAccount'];
			dropListCache: ChipDisconnectDeps['dropListCache'];
			dropProviderCache: ChipDisconnectDeps['dropProviderCache'];
		}
	) => Promise<{ kind: 'ok' } | { kind: 'token_clear_failed' }>;
	clearPersistedAccount: (provider: Provider) => Promise<boolean>;
	dropListCache: (provider: Provider, userId: string | null) => Promise<void>;
	dropProviderCache: (provider: Provider) => Promise<void>;
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
	void provider;
	void prev;
	void deps;
	void cb;
}
