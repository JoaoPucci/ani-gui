/**
 * Stale-upstream-URL recovery policy for the play page.
 *
 * Allmanga (and the other allanime mirrors we resolve through) rotate
 * the byte-stream URL behind their CDN tokens on a short horizon. If
 * the user pauses, the laptop sleeps, then they come back hours later
 * and click play, the URL they're sitting on is dead — the <video>
 * element fires `error` with code=2 (MEDIA_ERR_NETWORK) or hls.js
 * fatals with `data.type === 'networkError'`.
 *
 * The page recovers by evicting the cached resolution and re-running
 * playStream. The decision of *whether to attempt that silent recovery*
 * lives here so it's pure and testable. The recovery flow itself stays
 * in +page.svelte (it needs component state — detail, config,
 * episodeNum — that isn't worth threading through a helper).
 *
 * Policy:
 *   • Only network-class errors are URL-rotation symptoms.
 *     Decode / not-supported / aborted aren't fixed by refetching, and
 *     trying would waste a cache eviction.
 *   • One auto-retry per session. If the silent retry itself fails
 *     (fresh URL is also bad, or a different upstream problem), the
 *     second error surfaces — the page shows the player-error overlay
 *     with the manual Reload button as the escape hatch.
 *
 * The cacheHit-only gate this replaced was over-narrow: it only fired
 * the recovery when the user landed on the page via a SQLite cache hit
 * (`?cache_hit=1`). Fresh resolves (no cache_hit) and second-time
 * stale URLs in the same session both fell through to a dead-end
 * `playerError` string. Widening the gate catches both cases.
 */

export type StreamFailure =
	| { source: 'video'; code: number; message?: string }
	| { source: 'hls'; type: string; details?: string };

/** True when the failure is the rotated-URL signature: <video>
 *  MEDIA_ERR_NETWORK (code 2) or hls.js fatal networkError. */
export function isNetworkClassStreamError(err: StreamFailure): boolean {
	if (err.source === 'video') return err.code === 2;
	return err.type === 'networkError';
}

/** True when the page should silently evict + refetch instead of
 *  surfacing the error. False means surface to the user via the
 *  player-error overlay (and the manual Reload button takes over). */
export function shouldAttemptStaleStreamRetry(args: {
	err: StreamFailure;
	hasAutoRetried: boolean;
}): boolean {
	return !args.hasAutoRetried && isNetworkClassStreamError(args.err);
}

/** True when a successful switchToEpisode landing should reset the
 *  one-shot auto-retry budget. Resets only on user-driven episode
 *  changes (Next/Prev/pick — distinct session-class, distinct budget).
 *
 *  Internal auto-recovery calls switchToEpisode with the *current*
 *  episode to swap the rotated URL — those must NOT reset, otherwise
 *  the next stale URL re-triggers the silent retry and loops without
 *  bound. The manual Reload button also targets the current episode,
 *  so it inherits the no-reset behaviour: the user gets one auto-
 *  retry per session, then it's manual clicks until they pick a
 *  different episode (or navigate away).
 */
export function shouldResetStaleStreamBudget(args: {
	currentEpisode: number;
	targetEpisode: number;
}): boolean {
	return args.targetEpisode !== args.currentEpisode;
}

/** True when the page has the state it needs to actually run the
 *  recovery flow. Only the kitsu detail row is hard-required — its
 *  canonical_title + alt_titles + episode_count + year populate the
 *  eviction payload, and there's no sensible fallback. Settings is
 *  intentionally NOT required: the recovery flow falls back to
 *  sub/best (matching switchToEpisode's `config?.mode` /
 *  `config?.quality` pattern), so a permanently null config from a
 *  rejected settingsGet shouldn't disable the Reload button.
 *
 *  When this returns false, the auto-retry path falls through to
 *  playerError instead of consuming hasAutoRetried on a no-op
 *  recovery; the user sees the overlay, and clicking Reload once
 *  detail loads runs the recovery successfully. */
export function canRecoverFromStaleStream(args: { detail: unknown }): boolean {
	return args.detail !== null;
}
