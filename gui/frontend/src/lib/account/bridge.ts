/**
 * Thin wrappers around the Electron preload bridge surfaces the
 * account integration uses — safeStorage CRUD, OAuth flow control, and
 * `openExternal` for the Privacy Policy link.
 *
 * Extracted from `api.ts` so the HTTP-call helpers and the bridge
 * helpers each have their own file. Keeps both modules' CCN below the
 * CRAP ratchet ceiling (`coverage-baseline.json#crap.high_risk_le`).
 */

import type { OAuthOpenResult, PersistedAccount, Provider } from './types';

function bridge() {
	return (typeof window !== 'undefined' ? window : undefined)?.aniGui?.account;
}

/**
 * Result of a safeStorage read. Codex P2 #3371530183: collapsing every
 * non-ok read to `null` made `hydrate()` treat a keychain outage the
 * same as "no token on disk", hiding the Disconnect action even though
 * the credential file was still there. Now the consumer can distinguish
 * `not_found` (genuinely no account → disconnected) from `unreadable`
 * (file present but keychain broken / decrypt failed → error state
 * exposing a Clear-local-file affordance).
 */
export type ReadPersistedAccountResult =
	| { ok: true; account: PersistedAccount }
	| { ok: false; kind: 'not_found' }
	| { ok: false; kind: 'unreadable'; detail: string };

/** Synchronous safeStorage read. Used at boot by `accountStore.hydrate()`. */
export function readPersistedAccount(provider: Provider): ReadPersistedAccountResult {
	const b = bridge();
	if (!b) return { ok: false, kind: 'not_found' };
	const r = b.getToken(provider);
	if (r.ok) return { ok: true, account: r.payload };
	// `not_found` is the only "you should treat this as disconnected"
	// case. Everything else means the file is on disk but unusable —
	// surface it so the page can render a cleanup affordance instead
	// of pretending no account exists.
	if (r.kind === 'not_found') return { ok: false, kind: 'not_found' };
	return { ok: false, kind: 'unreadable', detail: r.kind };
}

/** Encrypt + write to safeStorage. Returns whether the write succeeded. */
export async function persistAccount(
	provider: Provider,
	payload: PersistedAccount
): Promise<boolean> {
	const b = bridge();
	if (!b) return false;
	const r = await b.setToken(provider, payload);
	return r.ok;
}

/** Drop the persisted file on disk. Returns whether the delete succeeded. */
export async function clearPersistedAccount(provider: Provider): Promise<boolean> {
	const b = bridge();
	if (!b) return false;
	const r = await b.clearToken(provider);
	return r.ok;
}

/**
 * Open the provider's OAuth consent page in the OS browser and wait
 * for the loopback callback to fire. Resolves with the
 * `OAuthOpenResult` the preload sends back.
 */
export async function openOAuth(authUrl: string): Promise<OAuthOpenResult> {
	const b = bridge();
	if (!b) {
		return {
			ok: false,
			kind: 'no_bridge',
			message: 'Electron preload is missing the account surface'
		};
	}
	return b.openOAuth({ authUrl });
}

/** Cancel an in-flight OAuth attempt. Returns whether the cancel was accepted. */
export async function cancelOAuth(): Promise<boolean> {
	const b = bridge();
	if (!b) return false;
	return b.cancelOAuth();
}

/** Open an https URL in the OS browser. Returns whether the call succeeded. */
export async function openExternal(url: string): Promise<boolean> {
	const fn = (typeof window !== 'undefined' ? window : undefined)?.aniGui?.openExternal;
	if (!fn) return false;
	return fn(url);
}
