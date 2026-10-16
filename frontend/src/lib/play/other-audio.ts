/**
 * What the app already knows about the other audio of a show: for a
 * play that just failed with the episode verdict (the show was found
 * and this episode has no stream in the chosen audio), and for the
 * detail page's notice when the chosen audio has nothing.
 *
 * The episode-level answer decides whether the failure copy suggests
 * switching audio (see `describePlayFailure`); the show-level one
 * decides whether the notice names the other audio
 * ($lib/detail/unavailable-copy). Both come from the availability
 * cache only: the batch route reads cached rows and asks the provider
 * nothing, so neither ever starts a probe of its own. Anything short
 * of a cached answer is "unknown", and unknown claims nothing.
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
 *  (a range download), which leaves the answer unknown. */
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

/** The other audio's cached row for `kitsuId`: whether it is
 *  available and its count, if any. `null` when there is no id, no
 *  row, or the lookup failed or stalled. */
async function readOtherAudioRow(
	kitsuId: string | null | undefined,
	mode: string,
	batch: Batch
): Promise<{ available: boolean; count: number | null } | null> {
	if (!kitsuId) return null;
	try {
		const res = await withTimeout(batch([kitsuId], otherAudioMode(mode)), LOOKUP_TIMEOUT_MS);
		const available = res?.cached?.[kitsuId];
		if (typeof available !== 'boolean') return null;
		const count = res?.playable_episode_counts?.[kitsuId];
		return { available, count: typeof count === 'number' ? count : null };
	} catch {
		return null;
	}
}

/** The cache's answer for the other audio of `q`'s show and
 *  episode. `true` only on evidence for the episode: a positive row
 *  whose count covers it. `false` when the row says the other audio
 *  has nothing, or its count stops short of the episode — the other
 *  audio's pages gate episodes by that count, so the episode would be
 *  disabled there. `null` otherwise: no row, a positive row without a
 *  count (a dub resolve stamps those; they prove the show, not the
 *  episode), no episode named (a range download), or a lookup that
 *  failed or stalled. */
export async function lookupOtherAudio(
	q: OtherAudioQuery,
	batch: Batch = availabilityBatch
): Promise<OtherAudio> {
	const row = await readOtherAudioRow(q.kitsuId, q.mode, batch);
	if (row === null) return null;
	if (!row.available) return false;
	if (row.count === null || q.episode === null) return null;
	return q.episode <= row.count;
}

/** The cache's answer for whether the other audio carries the show
 *  at all — for a claim about the title, not an episode. `true` on a
 *  positive row, with or without a count (a positive row proves the
 *  show), unless its count is zero; `false` when the row says the
 *  other audio has nothing; `null` otherwise, a failed or stalled
 *  lookup included. */
export async function lookupOtherAudioShow(
	q: { kitsuId: string | null | undefined; mode: string },
	batch: Batch = availabilityBatch
): Promise<OtherAudio> {
	const row = await readOtherAudioRow(q.kitsuId, q.mode, batch);
	if (row === null) return null;
	if (!row.available) return false;
	return row.count === 0 ? null : true;
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
