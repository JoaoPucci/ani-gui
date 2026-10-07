/**
 * `describeError`: the user-facing sentence for any failure, chosen by
 * the AniError envelope's `kind` and never by its `detail`, which the
 * backend documents as free text for logs. Kinds not listed here, and
 * thrown values that are not an envelope, get the generic sentence.
 * Re-exported from ./error-copy beside the surface-specific mappers.
 */

import { m } from '$lib/paraglide/messages';

const REASON_BY_KIND: Readonly<Record<string, () => string>> = {
	network: m.errors_reason_network,
	gate_refused: m.errors_reason_network,
	upstream: m.errors_reason_network,
	http: m.errors_reason_network,
	timeout: m.errors_reason_timeout,
	rate_limited: m.errors_reason_busy,
	parse_failed: m.errors_reason_bad_response,
	metadata: m.errors_reason_bad_response,
	cache: m.errors_reason_local,
	io: m.errors_reason_local,
	config: m.errors_reason_local
};

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
	const reason = kind !== null && Object.hasOwn(REASON_BY_KIND, kind) ? REASON_BY_KIND[kind] : null;
	return (reason ?? m.errors_reason_generic)();
}
