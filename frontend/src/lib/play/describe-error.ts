/**
 * `describeError`: the user-facing sentence for any failure, chosen
 * from the AniError envelope and never from its `detail`, which the
 * backend documents as free text for logs.
 *
 * Three lookups, in order:
 *
 *   1. A field that says more than any one sentence per key could: an
 *      upstream's 404/410 (not found), 429 (busy) or 5xx (the service
 *      is down), and a rate limit's advertised wait.
 *   2. The envelope's `key` — the backend's stable name for exactly
 *      what failed (`error.metadata.source`). Each key has one message
 *      per locale in `messages/<locale>/backend.json`, and the message
 *      may interpolate the envelope's structured fields (an HTTP
 *      status, the binary a spawn failed on). `tests/arch/error_keys.sh`
 *      holds those catalogues to the backend's published key list.
 *   3. The envelope's `kind` — the variant's coarser grouping, for an
 *      envelope with no key, a key this build has no message for, or
 *      a keyed message whose field the envelope does not carry.
 *
 * Kinds not listed either, and thrown values that are not an envelope,
 * get the generic sentence. Re-exported from ./error-copy beside the
 * surface-specific mappers.
 */

import { m } from '$lib/paraglide/messages';

type Envelope = Record<string, unknown>;

/** The message for a key, or null when the envelope lacks a field
 *  the message needs — the kind's sentence is better than "undefined"
 *  or an empty name in the middle of one. */
type KeyedMessage = (e: Envelope) => string | null;

const withStatus =
	(message: (i: { status: number }) => string): KeyedMessage =>
	(e) =>
		typeof e.status === 'number' ? message({ status: e.status }) : null;

const withBinary =
	(message: (i: { binary: string }) => string): KeyedMessage =>
	(e) =>
		typeof e.binary === 'string' && e.binary.length > 0 ? message({ binary: e.binary }) : null;

/** One entry per backend error key. `describe-error.test.ts` holds
 *  this table to `backend/error-keys.json`, so a key the backend adds
 *  fails there until it has an entry here and a message per locale. */
const MESSAGE_BY_KEY: Readonly<Record<string, KeyedMessage>> = {
	'error.account.unsupported_pkce': m.backend_error_account_unsupported_pkce,
	'error.cache.generic': m.backend_error_cache_generic,
	'error.config.parse': m.backend_error_config_parse,
	'error.download.ffmpeg_missing': m.backend_error_download_ffmpeg_missing,
	'error.download.tool_failed': m.backend_error_download_tool_failed,
	'error.download.tool_spawn_failed': m.backend_error_download_tool_spawn_failed,
	'error.io.generic': m.backend_error_io_generic,
	'error.metadata.source': m.backend_error_metadata_source,
	'error.network.rate_limited': m.backend_error_network_rate_limited,
	'error.network.unreachable': m.backend_error_network_unreachable,
	'error.network.upstream': withStatus(m.backend_error_network_upstream),
	'error.play.episode_unavailable': m.backend_error_play_episode_unavailable,
	'error.player.spawn_failed': withBinary(m.backend_error_player_spawn_failed),
	'error.request.invalid_kitsu_id': m.backend_error_request_invalid_kitsu_id,
	'error.scraper.parse_failed': m.backend_error_scraper_parse_failed,
	'error.scraper.timeout': m.backend_error_scraper_timeout,
	'error.search.no_results': m.backend_error_search_no_results,
	'error.stream.invalid_token': m.backend_error_stream_invalid_token,
	'error.syncplay.spawn_failed': withBinary(m.backend_error_syncplay_spawn_failed)
};

const REASON_BY_KIND: Readonly<Record<string, () => string>> = {
	network: m.errors_reason_network,
	gate_refused: m.errors_reason_network,
	upstream: m.errors_reason_answered_error,
	http: m.errors_reason_answered_error,
	no_results: m.errors_reason_not_found,
	timeout: m.errors_reason_timeout,
	rate_limited: m.errors_reason_busy,
	parse_failed: m.errors_reason_bad_response,
	metadata: m.errors_reason_bad_response,
	cache: m.errors_reason_local,
	io: m.errors_reason_local,
	config: m.errors_reason_local
};

/** The AniError envelope, or null for anything that is not an object. */
function envelopeOf(e: unknown): Envelope | null {
	return typeof e === 'object' && e !== null ? (e as Envelope) : null;
}

/** The reading of a field that says more than the key: an upstream
 *  status of 404/410, 429 or 5xx, a rate limit's wait. Null for every
 *  other envelope, including an upstream status none of those claim. */
function fromFields(env: Envelope): string | null {
	if (env.kind === 'rate_limited') {
		const secs = env.retry_after_secs;
		return typeof secs === 'number' ? m.errors_reason_busy_wait({ seconds: secs }) : null;
	}
	if (env.kind !== 'upstream' || typeof env.status !== 'number') return null;
	const status = env.status;
	if (status === 404 || status === 410) return m.errors_reason_not_found();
	if (status === 429) return m.errors_reason_busy();
	if (status >= 500) return m.errors_reason_service_down();
	return null;
}

/** The key's message, or null when the envelope has no key this build
 *  has a message for — or has one and lacks a field it needs. */
function byKey(env: Envelope): string | null {
	const key = env.key;
	if (typeof key !== 'string' || !Object.hasOwn(MESSAGE_BY_KEY, key)) return null;
	return MESSAGE_BY_KEY[key](env);
}

/** The kind's sentence, or null for a kind without one. */
function byKind(env: Envelope): string | null {
	const kind = env.kind;
	if (typeof kind !== 'string' || !Object.hasOwn(REASON_BY_KIND, kind)) return null;
	return REASON_BY_KIND[kind]();
}

/** User-facing copy for any failure: a field's more specific reading,
 *  else the key's localized message, else the kind's sentence, else
 *  the generic one — never the detail. */
export function describeError(e: unknown): string {
	const env = envelopeOf(e);
	if (env === null) return m.errors_reason_generic();
	return fromFields(env) ?? byKey(env) ?? byKind(env) ?? m.errors_reason_generic();
}
