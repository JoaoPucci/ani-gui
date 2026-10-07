/**
 * Maps backend errors to user-facing copy. Three helpers, in order of
 * specificity:
 *
 *   • `describeError` is the general one: a short localized sentence
 *     naming the cause by the error's `kind`. Every surface that shows
 *     a failure without a copy of its own uses it.
 *   • `describePlayFailure` picks the right user-facing message for
 *     a play-call failure — "no episode," "scraper unhappy,"
 *     "network trouble," etc.
 *   • `describeExternalLaunchFailure` covers "Open in external player".
 *
 * None of them prints the payload's `detail`. The backend documents it
 * as free text for logs (ParseFailed carries serde's message, a URL that
 * failed validation, …), and a raw `kind` token or a thrown Error's
 * message is no better on screen.
 *
 * Extracted from the play page so the message branches can be
 * unit-tested instead of being threaded through Svelte effect
 * runtime.
 */

import { m } from '$lib/paraglide/messages';

/** The `kind` of an AniError envelope, or null for anything else. */
function kindOf(e: unknown): string | null {
	if (typeof e !== 'object' || e === null) return null;
	const kind = (e as Record<string, unknown>).kind;
	return typeof kind === 'string' ? kind : null;
}

/** Flatten a thrown value into the lowercase text `describePlayFailure`
 *  matches its branches against. Internal: it can carry the payload's
 *  detail, so it is for classification only and never shown. */
function classifierText(e: unknown): string {
	if (typeof e === 'object' && e !== null) {
		const obj = e as Record<string, unknown>;
		const kind = typeof obj.kind === 'string' ? obj.kind : null;
		const detail = typeof obj.detail === 'string' ? obj.detail : null;
		if (kind && detail) return `${kind}: ${detail}`.toLowerCase();
		if (kind) return kind.toLowerCase();
	}
	return String(e).toLowerCase();
}

/** User-facing copy for any failure: a localized sentence chosen by
 *  the error's kind, never its detail. Unrecognised kinds and values
 *  that are not an AniError envelope get the generic sentence. */
export function describeError(e: unknown): string {
	switch (kindOf(e)) {
		case 'network':
		case 'gate_refused':
		case 'upstream':
		case 'http':
			return m.errors_reason_network();
		case 'timeout':
			return m.errors_reason_timeout();
		case 'rate_limited':
			return m.errors_reason_busy();
		case 'parse_failed':
		case 'metadata':
			return m.errors_reason_bad_response();
		case 'cache':
		case 'io':
		case 'config':
			return m.errors_reason_local();
		default:
			return m.errors_reason_generic();
	}
}

/** First-chance mapper for the backend's typed rate limit. Returns
 *  the busy-source copy — with the upstream's advertised wait when
 *  it sent one ("try again in N seconds" → retry_after_secs) — or
 *  `null` for every other error. The detail and home pages keep
 *  their own surface-specific mappers for the older kinds; they call
 *  this first so all play surfaces share one localized rate-limit
 *  branch instead of each growing a divergent copy. */
export function describeRateLimit(e: unknown): string | null {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	if (obj?.kind !== 'rate_limited') return null;
	const secs = obj.retry_after_secs;
	return typeof secs === 'number'
		? m.play_play_failure_rate_limited_wait({ seconds: secs })
		: m.play_play_failure_rate_limited();
}

/** Shared first-chance mapper for the provider being down: upstream
 *  5xx — the provider explicitly answering "service unavailable",
 *  maintenance or an outage — returns copy that blames the source
 *  and clears the user's own setup; null for everything else. The
 *  detail and home pages keep surface-specific mappers for the
 *  older kinds, so like the rate-limit branch this must be called
 *  by each of them, not folded into one mapper of three. */
export function describeSourceDown(e: unknown): string | null {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	if (obj?.kind === 'upstream' && typeof obj.status === 'number' && obj.status >= 500) {
		return m.play_play_failure_source_down();
	}
	return null;
}

/** User-facing copy for a play-call failure. The message branches
 *  match (in order): rate_limited → busy source (with the upstream's
 *  own retry hint when it sent one); episode_unavailable → the show
 *  is there and this episode is not; no_results → catalogue miss;
 *  scraper → upstream unhappy; timeout → slow upstream; network /
 *  upstream → connection trouble; default → generic retry. */
export function describePlayFailure(e: unknown, opts?: { noResults?: () => string }): string {
	const rateLimited = describeRateLimit(e);
	if (rateLimited !== null) return rateLimited;
	const sourceDown = describeSourceDown(e);
	if (sourceDown !== null) return sourceDown;
	const raw = classifierText(e);
	if (raw.includes('episode_unavailable')) {
		// The show is in the catalogue; this episode has no stream in
		// the requested audio. The same copy on every surface — the
		// detail page's catalogue-miss phrasing is for a title the
		// catalogue lacks, which this is not.
		return m.play_play_failure_episode_unavailable();
	}
	if (raw.includes('no_results')) {
		// The one deliberate per-surface difference: the detail page
		// phrases a catalogue miss definitively (it also gates the
		// Play CTA proactively); every other surface keeps the hedge.
		return (opts?.noResults ?? m.play_play_failure_no_results)();
	}
	if (raw.includes('scraper')) {
		return m.play_play_failure_scraper();
	}
	if (raw.includes('timeout')) {
		return m.play_play_failure_timeout();
	}
	if (raw.includes('network') || raw.includes('upstream')) {
		return m.play_play_failure_network();
	}
	return m.play_play_failure_generic();
}

/** User-facing copy for an "Open in external player" failure. The
 *  common case is a `player_spawn_failed` payload — the configured
 *  binary isn't on PATH or doesn't exist. Other resolve-step
 *  failures (scraper / timeout / network) reuse `describePlayFailure`
 *  so the user doesn't get a debug-y "External player failed:
 *  scraper" string.
 *
 *  Returns the body text only — the surrounding modal's headline
 *  comes from `play_error_external_headline` and is interpolated
 *  with the episode number by the caller. */
export function describeExternalLaunchFailure(e: unknown): string {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	if (
		obj &&
		obj.kind === 'player_spawn_failed' &&
		typeof obj.binary === 'string' &&
		obj.binary.length > 0
	) {
		return m.play_external_spawn_failed_named({ binary: obj.binary });
	}
	// Other failures (scraper / timeout / network on the resolve
	// step) reuse the embedded path's copy so the user sees a
	// polished message instead of a debug-y "External player
	// failed: <kind>".
	return describePlayFailure(e);
}
