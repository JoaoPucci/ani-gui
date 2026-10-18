/**
 * The detail page's warm of its resume episode: a resolve that records
 * no watch, asked when no listing stood behind the page's lookup and
 * the resume row reads as a fractional episode — the one kind a
 * renumbering leaves under a number it no longer has. Resolving it
 * fetches the listing, which puts the row right, and the page reads the
 * row again after ({@link refreshedResume}).
 */

import type { CreateSessionResponse, PlayArgs, PlayProgress } from '$lib/api';
import { getOrFireNow, makeKey } from '$lib/play/play-cache';

/** The play-cache axes of the episode warmed. */
export interface ResumeWarmKey {
	showId: string;
	episode: number;
	mode: string;
	quality: string;
}

/** `playStream`'s shape, injected so the warm can be tested. */
export type ResumeWarmStream = (
	args: PlayArgs,
	onProgress: (p: PlayProgress) => void,
	signal?: AbortSignal
) => Promise<CreateSessionResponse>;

/** Resolves `args` as a prefetch, through the play cache.
 *
 *  Through the cache, the page's unmount cancels it with the page's
 *  other prefetches (`clearForShow`), so it does not hold a connection
 *  on the resolve lane after its page; a move to another show on the
 *  same page lets it run on, as it does the show's other warms. A click
 *  on the same episode plays the session it resolved instead of asking
 *  again.
 *
 *  It starts at once rather than behind the next episodes' background
 *  warms, since a click on Play waits for the row this warm puts right.
 *  It stays a background entry all the same: a click that attaches
 *  while it runs fires its own Interactive resolve, as for any
 *  prefetch, rather than waiting on one the backend paces as
 *  background. */
export function warmResumeEpisode(
	key: ResumeWarmKey,
	args: PlayArgs,
	stream: ResumeWarmStream
): Promise<unknown> {
	return getOrFireNow(makeKey(key.showId, key.episode, key.mode, key.quality), (emit, signal) =>
		stream({ ...args, prefetch: true }, emit, signal)
	);
}
