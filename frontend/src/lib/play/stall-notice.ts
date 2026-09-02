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
import { isNetworkClassStreamError, type StreamFailure } from '$lib/play/stale-stream';
import { m } from '$lib/paraglide/messages';

/** The host-struggling signature: hls.js timeout / stall details.
 *  The playlists loaded (hls.js got far enough to time out on
 *  media) while segments crawl — the provider shard, not the link. */
const HOST_SLOW = /timeout|stalled/i;

export type StallCause = 'host-slow' | 'link-stale';

/** Classify a recovery reason string (`hls fragLoadTimeOut`,
 *  `video MEDIA_ERR_NETWORK (…)`, …). */
export function classifyStallCause(reason: string): StallCause {
	return HOST_SLOW.test(reason) ? 'host-slow' : 'link-stale';
}

/** The toast announcing a silent recovery — or null when the user
 *  started it themselves (manual reload: they already know). */
export function stallRecoveryToast(reason: string): PushArgs | null {
	if (reason === 'manual reload') return null;
	return {
		kind: 'info',
		message:
			classifyStallCause(reason) === 'host-slow'
				? m.play_stall_toast_host_slow()
				: m.play_stall_toast_link_stale()
	};
}

/** Cause-naming overlay copy once the retry budget is spent and the
 *  failure still carries the host-slow signature; null keeps the
 *  caller's generic message. */
export function exhaustedStallOverlayMessage(
	err: StreamFailure,
	hasAutoRetried: boolean
): string | null {
	if (!hasAutoRetried || !isNetworkClassStreamError(err)) return null;
	if (err.source !== 'hls') return null;
	return HOST_SLOW.test(err.details ?? '') ? m.play_error_host_slow() : null;
}
