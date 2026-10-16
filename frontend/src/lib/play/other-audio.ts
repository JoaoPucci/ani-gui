/**
 * What the app already knows about the other audio of a show whose
 * play just failed with the episode verdict — the show was found and
 * this episode has no stream in the chosen audio.
 *
 * The answer decides whether the failure copy suggests switching
 * audio (see `describePlayFailure`). It comes from the availability
 * cache only: the batch route reads cached rows and asks the provider
 * nothing, so a failure never starts a probe of its own. Anything
 * short of a cached answer is "unknown", and unknown does not suggest.
 */

import { availabilityBatch } from '$lib/api';
import {
	describePlayFailure,
	isEpisodeUnavailable,
	type OtherAudio,
	type PlayFailureContext
} from './error-copy';

/** The play that failed: the show's Kitsu id, the audio it asked
 *  for, and its episode — `null` when the surface cannot name one
 *  (a range download). */
export interface OtherAudioQuery {
	kitsuId: string | null | undefined;
	mode: string;
	episode: number | null;
}

type Batch = typeof availabilityBatch;

/** How long a failure surface waits on the cache before showing its
 *  message without the answer. The lookup is a local read, so this
 *  only bounds a sidecar that stopped answering. */
const LOOKUP_TIMEOUT_MS = 2000;

/** The audio the user is not playing. The play path reads anything
 *  but "dub" as sub. */
export function otherAudioMode(mode: string): 'sub' | 'dub' {
	return mode === 'dub' ? 'sub' : 'dub';
}

function withTimeout<T>(p: Promise<T>, ms: number): Promise<T | null> {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => resolve(null), ms);
		p.then(
			(v) => {
				clearTimeout(timer);
				resolve(v);
			},
			(err: unknown) => {
				clearTimeout(timer);
				reject(err instanceof Error ? err : new Error(String(err)));
			}
		);
	});
}

/** The cache's answer for the other audio of `q`'s show: `true` when
 *  it has the show and no count it carries falls short of the episode,
 *  `false` when it has nothing or a count short of the episode, `null`
 *  when it holds no answer or the lookup fails or stalls. The cached
 *  count is what the other audio's pages gate episodes by, so an
 *  episode past it would be disabled there; suggesting the switch
 *  would land the user on it. */
export async function lookupOtherAudio(
	q: OtherAudioQuery,
	batch: Batch = availabilityBatch
): Promise<OtherAudio> {
	const id = q.kitsuId;
	if (!id) return null;
	try {
		const res = await withTimeout(batch([id], otherAudioMode(q.mode)), LOOKUP_TIMEOUT_MS);
		const cached = res?.cached?.[id];
		if (cached === false) return false;
		if (cached !== true) return null;
		const count = res?.playable_episode_counts?.[id];
		if (typeof count === 'number' && q.episode !== null && q.episode > count) return false;
		return true;
	} catch {
		return null;
	}
}

/** `lookupOtherAudio`, asked only for the episode verdict — the one
 *  failure whose copy depends on it. Every other failure answers
 *  unknown without a request. */
export function otherAudioForFailure(
	e: unknown,
	q: OtherAudioQuery,
	batch: Batch = availabilityBatch
): Promise<OtherAudio> {
	return isEpisodeUnavailable(e) ? lookupOtherAudio(q, batch) : Promise.resolve(null);
}

/** The failure copy for a play surface, with the other audio's
 *  answer looked up from the cache. `opts` carries the surface's own
 *  catalogue-miss phrasing, if any. */
export async function describePlayFailureFromCache(
	e: unknown,
	q: OtherAudioQuery,
	opts: Omit<PlayFailureContext, 'otherAudio'> = {},
	batch: Batch = availabilityBatch
): Promise<string> {
	const otherAudio = await otherAudioForFailure(e, q, batch);
	return describePlayFailure(e, { ...opts, otherAudio });
}
