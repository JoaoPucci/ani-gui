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

type Batch = typeof availabilityBatch;

/** The notice for a title unavailable in `mode`, given what the cache
 *  says about the other audio. Only `true` names the other audio. */
export function unavailableMessage(mode: string, otherAudio: OtherAudio): string {
	if (otherAudio !== true) return m.detail_unavailable_message();
	return mode === 'dub'
		? m.detail_unavailable_in_dub_message()
		: m.detail_unavailable_in_sub_message();
}

/** Ask the cache about the other audio of `q`'s title and hand the
 *  answer to `onAnswer`. Returns a cancel: an answer that lands after
 *  it is about a question the page no longer asks, and is dropped. */
export function watchOtherAudio(
	q: { kitsuId: string | null | undefined; mode: string },
	onAnswer: (answer: OtherAudio) => void,
	batch: Batch = availabilityBatch
): () => void {
	let live = true;
	void lookupOtherAudioShow(q, batch).then((answer) => {
		if (live) onAnswer(answer);
	});
	return () => {
		live = false;
	};
}
