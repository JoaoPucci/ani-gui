/**
 * Pure helpers behind the play page's "Watch together" (Syncplay)
 * surface. Mirrors `external-toast.ts` shape for parity: a
 * success-toast builder + a failure-copy helper. Both stay pure so
 * the play page's hamburger handler can be a thin adapter
 * (AGENTS.md §2).
 */

import type { PushArgs } from '$lib/toasts/store.svelte';
import { m } from '$lib/paraglide/messages';
import { describePlayFailure } from './error-copy';

/** Build the `PushArgs` for the success toast that announces a
 *  Syncplay launch on `episode`. Same 4s duration as the external-
 *  player success toast — both events feel the same from the user's
 *  perspective ("the click did something, watch the next window
 *  open"). */
export function syncplayLaunchSuccessToast(args: { episode: number }): PushArgs {
	return {
		kind: 'success',
		message: m.play_syncplay_sent_toast({ episode: args.episode }),
		duration: 4000
	};
}

/** User-facing copy for a Syncplay launch failure. The common case
 *  is a `syncplay_spawn_failed` payload — the configured binary
 *  isn't on PATH or doesn't exist; the surrounding modal then links
 *  the user to syncplay.pl. Other resolve-step failures (scraper /
 *  timeout / network) reuse `describePlayFailure` so the user sees
 *  the same polished message as the embedded play path.
 *
 *  Returns the body text only — the modal's headline and action
 *  link live on the play page (i18n keys + the syncplay.pl href). */
export function describeSyncplayLaunchFailure(e: unknown): string {
	const obj = typeof e === 'object' && e !== null ? (e as Record<string, unknown>) : null;
	if (
		obj &&
		obj.kind === 'syncplay_spawn_failed' &&
		typeof obj.binary === 'string' &&
		obj.binary.length > 0
	) {
		return m.play_syncplay_spawn_failed_named({ binary: obj.binary });
	}
	// Resolve-step failures (scraper / timeout / network) — reuse
	// the embedded play path's copy so the user sees a polished
	// message instead of a debug-y "Syncplay failed: <kind>".
	return describePlayFailure(e);
}
