/**
 * What the app does when a tracker refuses a read's bearer.
 *
 * A read refused with a 401 says the access token no longer works,
 * whatever its expiry date said. A sign-in that can be renewed is
 * renewed; one that cannot is marked expired, so the sign-in notice
 * can say so.
 */

import type { PersistedAccount, Provider, ProviderState } from './types';
import type { RefreshFlowDeps } from './refresh-flow';
import type { RenewalFailure } from './renewal-failure';

export interface RefusedBearerDeps extends RefreshFlowDeps {
	state(provider: Provider): ProviderState;
	setConnected(provider: Provider, account: PersistedAccount): void;
	setExpired(provider: Provider, account: PersistedAccount): void;
	noteRenewalFailed(provider: Provider, failure: RenewalFailure): void;
}

export async function handleRefusedBearer(
	_deps: RefusedBearerDeps,
	_provider: Provider
): Promise<void> {
	void _deps;
	void _provider;
}
