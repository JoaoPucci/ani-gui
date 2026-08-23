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
import { enqueueTokenWrite } from './token-write-queue';

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

/**
 * Result of a safeStorage write. Codex P2 #3372942245: collapsing every
 * failure to `false` hid `encryption_unavailable` and `basic_text` from
 * the page, so Linux users without a usable keyring saw the generic
 * "sign-in failed" error after a successful OAuth round-trip with no
 * pointer to the keyring as the actual problem. The discriminated
 * result lets `connectAccount` thread the kind through to a specific
 * actionable message.
 */
export type PersistAccountResult =
	| { ok: true }
	| {
			ok: false;
			kind: 'no_bridge' | 'encryption_unavailable' | 'bad_request' | 'io_error' | 'unknown';
			detail?: string;
	  };

/** Encrypt + write to safeStorage. Returns the IPC failure kind on error so the page can act on it. */
export async function persistAccount(
	provider: Provider,
	payload: PersistedAccount
): Promise<PersistAccountResult> {
	const b = bridge();
	if (!b) return { ok: false, kind: 'no_bridge' };
	// Serialize with any concurrent clearToken for this provider so a
	// boot refresh's write can't land after a racing disconnect's clear
	// (Codex P2 #3416883099).
	const r = await enqueueTokenWrite(provider, () => b.setToken(provider, payload));
	if (r.ok) return { ok: true };
	return { ok: false, kind: normaliseSetTokenKind(r.kind), detail: r.message };
}

function normaliseSetTokenKind(
	kind: string
): 'no_bridge' | 'encryption_unavailable' | 'bad_request' | 'io_error' | 'unknown' {
	switch (kind) {
		case 'encryption_unavailable':
		case 'bad_request':
		case 'io_error':
			return kind;
		default:
			return 'unknown';
	}
}

/** Drop the persisted file on disk. Returns whether the delete succeeded. */
export async function clearPersistedAccount(provider: Provider): Promise<boolean> {
	const b = bridge();
	if (!b) return false;
	// Same per-provider queue as persistAccount: a disconnect's clear
	// enqueued after a refresh's persist runs strictly after it, so the
	// file ends cleared rather than holding a resurrected token.
	const r = await enqueueTokenWrite(provider, () => b.clearToken(provider));
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
