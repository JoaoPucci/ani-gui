/**
 * First-chance mappers for the provider's typed answers: a rate limit
 * (in-band `rate_limited` or an upstream 429), the source being down
 * (upstream 5xx) and any other upstream status. describePlayFailure
 * calls all three before its own branches, so every surface that uses
 * it — and the download dock, which routes provider answers through it
 * and calls describeRateLimit itself — reads one answer the same way.
 * Re-exported from ./error-copy.
 */

import { m } from '$lib/paraglide/messages';

/** First-chance mapper for a rate limit: the in-band `rate_limited`
 *  (with the upstream's advertised wait when it sent one, "try again
 *  in N seconds" → retry_after_secs) or an upstream 429. Returns the
 *  busy-source copy, or `null` for every other error.
 *  describePlayFailure calls it first, and the download dock calls it
 *  directly before its own branches. */
export function describeRateLimit(e: unknown): string | null {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	// An upstream that throttles with HTTP 429 is a rate limit too; the
	// backend keeps it as `upstream` + status rather than `rate_limited`.
	if (obj?.kind === 'upstream' && obj.status === 429) return m.play_play_failure_rate_limited();
	if (obj?.kind !== 'rate_limited') return null;
	const secs = obj.retry_after_secs;
	return typeof secs === 'number'
		? m.play_play_failure_rate_limited_wait({ seconds: secs })
		: m.play_play_failure_rate_limited();
}

/** First-chance mapper for the provider being down: upstream 5xx —
 *  the provider explicitly answering "service unavailable",
 *  maintenance or an outage — returns copy that blames the source and
 *  clears the user's own setup; null for everything else.
 *  describePlayFailure calls it after describeRateLimit. */
export function describeSourceDown(e: unknown): string | null {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	if (obj?.kind === 'upstream' && typeof obj.status === 'number' && obj.status >= 500) {
		return m.play_play_failure_source_down();
	}
	return null;
}

/** First-chance mapper for every other answer from the source:
 *  an `upstream` whose status is neither the busy nor the down shape.
 *  The source answered, so the connection is fine — 404/410 is the
 *  source not having the thing (the catalogue-miss copy, never the
 *  detail page's definitive override, since a 404 does not prove a
 *  title absent from the catalogue), and any other status, or an
 *  upstream with no status, is the source answering with an error.
 *  Null for every other kind. Called after describeRateLimit and
 *  describeSourceDown, which take the 429 and 5xx shapes first. */
export function describeSourceAnswer(e: unknown): string | null {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	if (obj?.kind !== 'upstream') return null;
	if (obj.status === 404 || obj.status === 410) return m.play_play_failure_no_results();
	return m.play_play_failure_source_error();
}
