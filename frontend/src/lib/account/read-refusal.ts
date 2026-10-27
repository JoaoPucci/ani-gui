/**
 * What the app does when a tracker refuses a read's bearer.
 *
 * A read refused with a 401 says the access token no longer works,
 * whatever its expiry date said. A sign-in that can be renewed is
 * renewed; one that cannot is marked expired, so the sign-in notice
 * can say so.
 */

import type { PersistedAccount, Provider, ProviderState } from './types';
import { refreshAccount, type RefreshFlowDeps } from './refresh-flow';
import type { RenewalFailure } from './renewal-failure';

export interface RefusedBearerDeps extends RefreshFlowDeps {
	state(provider: Provider): ProviderState;
	/** True while the provider's account is being changed (a disconnect
	 *  under way). */
	changing(provider: Provider): boolean;
	setConnected(provider: Provider, account: PersistedAccount): void;
	setExpired(provider: Provider, account: PersistedAccount): void;
	noteRenewalFailed(provider: Provider, failure: RenewalFailure): void;
}

/**
 * Whether a refusal of `refusedBearer` is about the sign-in `state`
 * holds now. For now: whenever the sign-in is connected.
 */
export function refusalApplies(
	state: ProviderState,
	_changing: boolean,
	_refusedBearer: string
): state is Extract<ProviderState, { kind: 'connected' }> {
	void _changing;
	void _refusedBearer;
	return state.kind === 'connected';
}

export async function handleRefusedBearer(
	deps: RefusedBearerDeps,
	provider: Provider,
	refusedBearer: string
): Promise<void> {
	const state = deps.state(provider);
	// An expired sign-in's renewal has already run and been reported;
	// the read failing with its old bearer adds nothing.
	if (!refusalApplies(state, deps.changing(provider), refusedBearer)) return;
	const { account } = state;
	if (!account.refresh_token) {
		// No renewal is possible, so none was refused: the sign-in has
		// ended, which the expired-session notice says.
		deps.setExpired(provider, account);
		return;
	}
	const outcome = await refreshAccount(deps, provider, account);
	if (outcome.kind === 'refreshed') {
		deps.setConnected(provider, outcome.account);
		return;
	}
	// `superseded` when the account changed while renewing: the refused
	// bearer was the old one's, and the new sign-in owes the user no
	// notice. refreshAccount checks the generation for a failure too.
	if (outcome.kind !== 'failed') return;
	deps.setExpired(provider, account);
	deps.noteRenewalFailed(provider, outcome.failure);
}
