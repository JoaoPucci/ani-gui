/**
 * Copy for an "Open in external player" failure. Kept beside
 * `error-copy.ts`, whose play-failure mapper it falls back to, rather
 * than inside it, so neither file carries both surfaces' branching.
 */

import { m } from '$lib/paraglide/messages';
import { describePlayFailure } from './error-copy';

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
