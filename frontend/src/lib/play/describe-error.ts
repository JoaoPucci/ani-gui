/**
 * `describeError`: the user-facing sentence for any failure, chosen by
 * the AniError envelope's `kind` (and, for `upstream` and
 * `rate_limited`, the status or wait the payload carries) and never by
 * its `detail`, which the backend documents as free text for logs.
 * Kinds not listed here, and thrown values that are not an envelope,
 * get the generic sentence.
 * Re-exported from ./error-copy beside the surface-specific mappers.
 */

import { m } from '$lib/paraglide/messages';

const REASON_BY_KIND: Readonly<Record<string, () => string>> = {
	network: m.errors_reason_network,
	gate_refused: m.errors_reason_network,
	http: m.errors_reason_answered_error,
	no_results: m.errors_reason_not_found,
	timeout: m.errors_reason_timeout,
	parse_failed: m.errors_reason_bad_response,
	metadata: m.errors_reason_bad_response,
	cache: m.errors_reason_local,
	io: m.errors_reason_local,
	config: m.errors_reason_local
};

/** Field-reading copy for the kinds whose payload says more than the
 *  kind does: an upstream's HTTP status, a rate limit's advertised
 *  wait. Null for every other kind. */
function fromFields(kind: string | null, obj: Record<string, unknown>): string | null {
	if (kind === 'rate_limited') return busy(obj.retry_after_secs);
	if (kind !== 'upstream') return null;
	const status = typeof obj.status === 'number' ? obj.status : null;
	if (status === 404 || status === 410) return m.errors_reason_not_found();
	if (status === 429) return busy(null);
	if (status !== null && status >= 500) return m.errors_reason_service_down();
	return m.errors_reason_answered_error();
}

function busy(secs: unknown): string {
	return typeof secs === 'number'
		? m.errors_reason_busy_wait({ seconds: secs })
		: m.errors_reason_busy();
}

/** The `kind` of an AniError envelope, or null for anything else. */
function kindOf(e: unknown): string | null {
	if (typeof e !== 'object' || e === null) return null;
	const kind = (e as Record<string, unknown>).kind;
	return typeof kind === 'string' ? kind : null;
}

/** User-facing copy for any failure: a localized sentence chosen by
 *  the error's kind, never its detail. */
export function describeError(e: unknown): string {
	const kind = kindOf(e);
	const fielded = fromFields(kind, kind === null ? {} : (e as Record<string, unknown>));
	if (fielded !== null) return fielded;
	const reason = kind !== null && Object.hasOwn(REASON_BY_KIND, kind) ? REASON_BY_KIND[kind] : null;
	return (reason ?? m.errors_reason_generic)();
}
