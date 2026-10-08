/**
 * Maps backend errors to user-facing copy. The helpers, in order of
 * specificity:
 *
 *   • `describeError` (in ./describe-error, re-exported here) is the
 *     general one: a short localized sentence naming the cause by the
 *     error's `kind`. Every surface that shows a failure without a copy
 *     of its own uses it.
 *   • `describeRateLimit`, `describeSourceDown` and
 *     `describeSourceAnswer` are first-chance mappers for the typed
 *     provider answers (busy, down, any other status); every play
 *     surface and the download dock call them before their own
 *     branches, so one answer reads the same everywhere.
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

export { describeError } from './describe-error';

/** First-chance mapper for the backend's typed rate limit. Returns
 *  the busy-source copy — with the upstream's advertised wait when
 *  it sent one ("try again in N seconds" → retry_after_secs) — or
 *  `null` for every other error. The detail and home pages keep
 *  their own surface-specific mappers for the older kinds; they call
 *  this first so all play surfaces share one localized rate-limit
 *  branch instead of each growing a divergent copy. */
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

/** Shared first-chance mapper for every other answer from the source:
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

/** User-facing copy for a play-call failure. The message branches
 *  match (in order): rate_limited / upstream 429 → busy source (with
 *  the upstream's own retry hint when it sent one); upstream 5xx →
 *  source down; upstream 404/410 → catalogue miss; other upstream →
 *  the source answered with an error; episode_unavailable → the show
 *  is there and this episode is not; no_results → catalogue miss;
 *  scraper → upstream unhappy; timeout → slow upstream; network /
 *  gate_refused → connection trouble; default → generic retry. */
export function describePlayFailure(e: unknown, opts?: { noResults?: () => string }): string {
	const rateLimited = describeRateLimit(e);
	if (rateLimited !== null) return rateLimited;
	const sourceDown = describeSourceDown(e);
	if (sourceDown !== null) return sourceDown;
	const answered = describeSourceAnswer(e);
	if (answered !== null) return answered;
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
	if (raw.includes('network') || raw.includes('gate_refused')) {
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
