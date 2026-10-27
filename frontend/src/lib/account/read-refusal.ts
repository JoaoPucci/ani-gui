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
	setConnected(provider: Provider, account: PersistedAccount): void;
	setExpired(provider: Provider, account: PersistedAccount): void;
	noteRenewalFailed(provider: Provider, failure: RenewalFailure): void;
}

export async function handleRefusedBearer(
	deps: RefusedBearerDeps,
	provider: Provider
): Promise<void> {
	const state = deps.state(provider);
	// An expired sign-in's renewal has already run and been reported;
	// the read failing with its old bearer adds nothing.
	if (state.kind !== 'connected') return;
	const { account } = state;
	if (!account.refresh_token) {
		// No renewal is possible, so none was refused: the sign-in has
		// ended, which the expired-session notice says.
		deps.setExpired(provider, account);
		return;
	}
	const generation = deps.generation(provider);
	const outcome = await refreshAccount(deps, provider, account);
	if (outcome.kind === 'refreshed') {
		deps.setConnected(provider, outcome.account);
		return;
	}
	// The account changed while renewing: the refused bearer was the
	// old one's, and the new sign-in owes the user no notice.
	if (outcome.kind !== 'failed' || deps.generation(provider) !== generation) return;
	deps.setExpired(provider, account);
	deps.noteRenewalFailed(provider, outcome.failure);
}
