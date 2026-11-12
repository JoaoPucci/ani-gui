/**
 * The notice the detail page shows in place of Play when the audio
 * being played is cached as unavailable.
 *
 * "Not in the streaming catalogue" is only true when the other audio
 * has nothing either. When the availability cache has the title in
 * the other audio, the notice says so instead. The answer comes from
 * the cache only (see $lib/play/other-audio), never from a probe.
 */

import { availabilityBatch } from '$lib/api';
import { m } from '$lib/paraglide/messages';
import type { OtherAudio } from '$lib/play/error-copy';
import { lookupOtherAudioShow } from '$lib/play/other-audio';
import { beginVisit } from '$lib/navigation/visit';

type Batch = typeof availabilityBatch;

/** The notice for a title unavailable in `mode`, given what the cache
 *  says about the other audio. Only `true` names the other audio. */
export function unavailableMessage(mode: string, otherAudio: OtherAudio): string {
	if (otherAudio !== true) return m.detail_unavailable_message();
	return mode === 'dub'
		? m.detail_unavailable_in_dub_message()
		: m.detail_unavailable_in_sub_message();
}

/** Whether the notice asks the cache about the other audio: while it
 *  is up (`unavailable`), and never for a music title, which this page
 *  holds unavailable in both audios, nor for a title the provider
 *  refused by name (`refused`), which it lists under nothing named for
 *  the title in either audio — the notice then keeps "Not in the
 *  streaming catalogue". */
export function asksOtherAudio(state: {
	unavailable: boolean;
	music: boolean;
	refused: boolean;
}): boolean {
	return state.unavailable && !state.music && !state.refused;
}

/** Ask the cache about the other audio of `q`'s title and hand the
 *  answer to `onAnswer`. Returns a cancel: an answer that lands after
 *  it is about a question the page no longer asks, and is dropped, and
 *  a lookup still out is cancelled. */
export function watchOtherAudio(
	q: { kitsuId: string | null | undefined; mode: string },
	onAnswer: (answer: OtherAudio) => void,
	batch: Batch = availabilityBatch
): () => void {
	// Cancelling ends the lookup's visit: its request is cancelled too,
	// and an answer it still gives is dropped.
	const reads = beginVisit();
	void reads
		.keep(lookupOtherAudioShow(q, (ids, mode) => batch(ids, mode, { signal: reads.signal })))
		.then(onAnswer);
	return reads.end;
}
