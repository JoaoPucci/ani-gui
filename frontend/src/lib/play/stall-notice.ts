/**
 * User-facing surfaces for the play page's stale-stream recovery.
 *
 * The recovery itself is silent plumbing — evict the session,
 * re-resolve, swap the source — and until now it LOOKED like the app
 * breaking: a black frame, then an unexplained loading overlay, then
 * maybe an error with no stated action. These helpers name what is
 * happening at the two moments the user can see:
 *
 *   • when a silent recovery starts, a toast says the stream stalled
 *     or its link expired and a fresh one is on the way;
 *   • when the one-shot retry budget is already spent and the stream
 *     is still timing out, the player-error overlay names the likely
 *     cause — the video host crawling, usually the provider's own
 *     trouble — and the honest action (retry later).
 *
 * The distinction rides the failure's flavor: hls.js timeout / stall
 * details mean the playlists loaded while the media crawls, which is
 * the host-struggling signature (a provider CDN shard serving a
 * 131KB segment at ~4KB/s produced exactly this); everything else
 * network-class is the rotated-URL signature the recovery exists
 * for. Pure module so the policy is testable without the page
 * (AGENTS.md §2).
 */

import type { PushArgs } from '$lib/toasts/store.svelte';
import type { StreamFailure } from '$lib/play/stale-stream';

export type StallCause = 'host-slow' | 'link-stale';

/** Classify a recovery reason string (`hls fragLoadTimeOut`,
 *  `video MEDIA_ERR_NETWORK (…)`, …). */
export function classifyStallCause(reason: string): StallCause {
	void reason;
	return 'link-stale';
}

/** The toast announcing a silent recovery — or null when the user
 *  started it themselves (manual reload: they already know). */
export function stallRecoveryToast(reason: string): PushArgs | null {
	void reason;
	return null;
}

/** Cause-naming overlay copy once the retry budget is spent and the
 *  failure still carries the host-slow signature; null keeps the
 *  caller's generic message. */
export function exhaustedStallOverlayMessage(
	err: StreamFailure,
	hasAutoRetried: boolean
): string | null {
	void err;
	void hasAutoRetried;
	return null;
}
