/**
 * Pure helper deciding what the topbar AccountChip should render.
 * Source of truth is `accountStore.byProvider`; the chip itself only
 * does the rendering, so this helper is unit-testable without any
 * Svelte / DOM dependency.
 *
 * Logic:
 *   - Walk providers in priority order (AniList → MAL → InHouse,
 *     same as `MERGE_ORDER` in `./watch-later`).
 *   - First provider with a surviving identity wins, even if its
 *     session is expired or transiently erroring — the chip then
 *     renders a warning dot so the user can recover from the chip
 *     popover instead of digging through /account.
 *   - Disconnected / connecting / orphan-error states are skipped
 *     entirely; if every provider is in one of those states the
 *     chip stays hidden and the side-rail's /account link is the
 *     primary entry point.
 */

import type { PersistedAccount, Provider, ProviderState } from './types';

export type ChipWarning = 'expired' | 'error';

export type ChipState =
	| { kind: 'hidden' }
	| {
			kind: 'connected';
			provider: Provider;
			username: string;
			avatarUrl: string | null;
			warning: ChipWarning | null;
	  };

const PRIORITY: ReadonlyArray<Provider> = ['anilist', 'mal', 'inhouse'];

function connectedFrom(
	provider: Provider,
	account: PersistedAccount,
	warning: ChipWarning | null
): ChipState {
	return {
		kind: 'connected',
		provider,
		username: account.username,
		avatarUrl: account.avatar_url,
		warning
	};
}

export function chipDescriptor(
	byProvider: Record<Provider, ProviderState>,
	// eslint-disable-next-line @typescript-eslint/no-unused-vars
	primary?: Provider | null
): ChipState {
	for (const provider of PRIORITY) {
		const state = byProvider[provider];
		if (state.kind === 'connected') return connectedFrom(provider, state.account, null);
		if (state.kind === 'expired') return connectedFrom(provider, state.account, 'expired');
		if (state.kind === 'error' && state.account)
			return connectedFrom(provider, state.account, 'error');
	}
	return { kind: 'hidden' };
}
