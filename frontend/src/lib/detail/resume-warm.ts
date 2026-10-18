/**
 * The detail page's warm of its resume episode: a resolve that records
 * no watch, asked when no listing stood behind the page's lookup and
 * the resume row reads as a fractional episode — the one kind a
 * renumbering leaves under a number it no longer has. Resolving it
 * fetches the listing, which puts the row right, and the page reads the
 * row again after ({@link refreshedResume}).
 */

import type { CreateSessionResponse, PlayArgs, PlayProgress } from '$lib/api';

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

/** Resolves `args` as a prefetch. */
export function warmResumeEpisode(
	_key: ResumeWarmKey,
	args: PlayArgs,
	stream: ResumeWarmStream
): Promise<unknown> {
	return stream({ ...args, prefetch: true }, () => {});
}
