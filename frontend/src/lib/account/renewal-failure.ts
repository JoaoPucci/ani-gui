/**
 * What a failed sign-in renewal says about the sign-in.
 *
 * A renewal can fail because the tracker refused the refresh token
 * (`rejected`: only a new sign-in helps), because the tracker did not
 * answer (`unreachable`: the same sign-in may still work), or for a
 * reason the app cannot tell (`unknown`). The copy a notice shows is
 * chosen from this, so a notice says "reconnect" only when the
 * tracker refused.
 */

import { AccountApiError } from './api';

export type RenewalFailure = 'rejected' | 'unreachable' | 'unknown';

/** Per-provider renewal failure, `null` while none is outstanding. */
export type RenewalFailures = Record<'anilist' | 'mal' | 'inhouse', RenewalFailure | null>;

export const NO_RENEWAL_FAILURES: RenewalFailures = { anilist: null, mal: null, inhouse: null };

/** Keys under which the backend says it could not reach the tracker:
 *  no connection, a timeout, a rate limit. */
const UNREACHABLE_KEYS: ReadonlySet<string> = new Set([
	'error.network.unreachable',
	'error.scraper.timeout',
	'error.network.rate_limited'
]);

interface Envelope {
	key?: unknown;
	status?: unknown;
}

/** The keyed envelope an account route failed with, when its body is one. */
function envelopeOf(error: unknown): Envelope | null {
	if (!(error instanceof AccountApiError)) return null;
	try {
		const body: unknown = JSON.parse(error.detail);
		return typeof body === 'object' && body !== null ? (body as Envelope) : null;
	} catch {
		return null;
	}
}

/** Whether an upstream's own status says the tracker failed or is busy. */
function isOutageStatus(status: unknown): boolean {
	return typeof status === 'number' && (status >= 500 || status === 429);
}

/**
 * Read a failed renewal call as one of the three, from the backend's
 * stable key alone (never the error's text): its refusal key is
 * `rejected`; a key saying the tracker was not reached, or an upstream
 * that answered with a 5xx or 429, is `unreachable`; anything else,
 * including the local backend not answering at all, is `unknown`.
 */
export function classifyRenewalError(error: unknown): RenewalFailure {
	const envelope = envelopeOf(error);
	if (envelope === null) return 'unknown';
	const { key } = envelope;
	if (key === 'error.account.refresh_rejected') return 'rejected';
	if (typeof key === 'string' && UNREACHABLE_KEYS.has(key)) return 'unreachable';
	if (key === 'error.network.upstream' && isOutageStatus(envelope.status)) return 'unreachable';
	return 'unknown';
}

/** True when a read was refused because its bearer was: a 401 whose
 *  envelope names a refused token. */
export function isBearerRefused(error: unknown): boolean {
	return (
		error instanceof AccountApiError &&
		error.status === 401 &&
		envelopeOf(error)?.key === 'error.stream.invalid_token'
	);
}
