/**
 * What the user is told when a sign-in that answered did not succeed
 * (other than a failed save, which `connect-action` words itself).
 * Split from `connect-action` so each file stays well inside the CRAP
 * gate's per-file bar.
 */

import { m } from '$lib/paraglide/messages';
import type { ConnectFlowResult } from './connect-flow';
import { connectErrorKey } from './state-helpers';

/** The message for a failed sign-in's result. */
export function connectFailureMessage(
	r: Exclude<ConnectFlowResult, { kind: 'connected' | 'persist_failed' }>
): string {
	if (r.kind === 'state_mismatch') return m.account_connect_error_oauth_error();
	if (r.kind === 'oauth_error') return oauthErrorMessage(connectErrorKey(r.reason));
	if (r.kind === 'api_error' && r.status) {
		return `${m.account_connect_error_unknown()} (${r.status})`;
	}
	return m.account_connect_error_unknown();
}

function oauthErrorMessage(key: string): string {
	switch (key) {
		case 'port_busy':
			return m.account_connect_error_port_busy();
		case 'timeout':
			return m.account_connect_error_timeout();
		case 'cancelled':
			return m.account_connect_error_cancelled();
		case 'oauth_error':
			return m.account_connect_error_oauth_error();
		case 'no_bridge':
			return m.account_connect_error_no_bridge();
		default:
			return m.account_connect_error_unknown();
	}
}
