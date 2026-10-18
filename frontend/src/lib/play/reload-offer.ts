/**
 * The episodes a play cannot reach, as the player knows them.
 *
 * Every play path on the player — tile clicks, prev/next,
 * auto-play-next, and the Reload on the player-error overlay, which
 * re-resolves the episode on screen — goes through one switch, and
 * the switch refuses an episode in one of these states before it asks
 * the provider anything:
 *
 *   • `delisted` — the provider no longer lists the show in the
 *     chosen audio. A delisted show comes back without a count, so
 *     the cap check cannot catch it: the cap is whatever it already
 *     was.
 *   • `unaired` — the schedule says the episode has not aired.
 *   • `beyond-playable` — the episode is past the provider's playable
 *     count (catalogue lag).
 *
 * None of them is a state a retry leaves: each holds until the page
 * learns something new about the show. So the player-error overlay
 * offers its Reload only while `deadEnd` is null for the episode on
 * screen, and the automatic recovery, which is the same re-resolve,
 * starts only then too. Otherwise the overlay says which of these it
 * is, in the words the rest of the app uses for that state
 * (`deadEndMessage`), and sends the viewer to the show's page, where
 * the audio switch is.
 *
 * Every other failure keeps the Reload: a network failure or an
 * expired link (the fresh resolve gets a fresh URL), a decode or
 * unsupported-format failure and a stream the engine stopped for
 * looping on a fragment (the fresh resolve can land on another
 * stream), and a failure while the page has not yet heard whether
 * the provider lists the show (the switch may well take it). A webview
 * without HLS keeps it too, though only because nothing known refuses
 * the switch: a fresh resolve has no reason to land on another kind of
 * stream.
 *
 * The switch also holds while the airing lookup is out. That is not a
 * dead end — the lookup settles on its own — so the Reload stays on
 * screen, held until the lookup lands, and the automatic recovery
 * waits: started then, either would be declined.
 */

import { availabilityBatch } from '$lib/api';
import { asksOtherAudio } from '$lib/detail/unavailable-copy';
import { m } from '$lib/paraglide/messages';
import type { OtherAudio } from './error-copy';
import { lookupOtherAudio, lookupOtherAudioShow } from './other-audio';

type Batch = typeof availabilityBatch;

/** Why a play cannot reach the episode. */
export type DeadEnd = 'delisted' | 'unaired' | 'beyond-playable';

/** What the page knows about the episode a play asks for. */
export interface EpisodeStanding {
	/** Whether the provider lists the show in the chosen audio; `null`
	 *  until an answer lands. */
	listed: boolean | null;
	/** Whether the schedule calls the episode unaired. */
	unaired: boolean;
	/** Whether the episode is past the provider's playable count. */
	beyondPlayable: boolean;
}

/** The state that keeps a play from the episode, or `null` when
 *  nothing known does. An unknown listing claims nothing. */
export function deadEnd(s: EpisodeStanding): DeadEnd | null {
	if (s.listed === false) return 'delisted';
	if (s.unaired) return 'unaired';
	if (s.beyondPlayable) return 'beyond-playable';
	return null;
}

/** What the overlay says for `end`, given what the cache says about
 *  the other audio. The other audio is named only on evidence (`true`):
 *  for a delisted show, the detail page's notice with the audio switch
 *  placed on the show's page rather than below; for an episode past
 *  the count, the play-failure copy for an episode with no stream. */
export function deadEndMessage(end: DeadEnd, mode: string, otherAudio: OtherAudio): string {
	switch (end) {
		case 'delisted':
			if (otherAudio !== true) return m.detail_unavailable_message();
			return mode === 'dub'
				? m.play_error_stream_unavailable_in_dub()
				: m.play_error_stream_unavailable_in_sub();
		case 'unaired':
			return m.detail_ep_unaired_tooltip();
		case 'beyond-playable':
			return otherAudio === true
				? m.play_play_failure_episode_unavailable_try_other_audio()
				: m.play_play_failure_episode_unavailable();
	}
}

/** The show and episode a dead end is about, and what keeps the
 *  detail page from naming the other audio for a title. */
export interface DeadEndQuery {
	kitsuId: string | null | undefined;
	mode: string;
	episode: number;
	/** A music title, which the detail page holds unavailable in both
	 *  audios. */
	music: boolean;
	/** The provider refused the title by name. */
	refused: boolean;
}

/** Ask the cache about the other audio for `end` and hand the answer
 *  to `onAnswer`: for a delisted show whether the other audio has the
 *  title (asked only where the detail page would ask), for an episode
 *  past the count whether it has the episode, and nothing for an
 *  unaired one. Returns a cancel: an answer that lands after it is
 *  about a question the page no longer asks, and is dropped. */
export function watchDeadEndOtherAudio(
	end: DeadEnd,
	q: DeadEndQuery,
	onAnswer: (answer: OtherAudio) => void,
	batch: Batch = availabilityBatch
): () => void {
	let ask: Promise<OtherAudio> | null = null;
	if (end === 'delisted' && asksOtherAudio({ unavailable: true, ...q })) {
		ask = lookupOtherAudioShow(q, batch);
	} else if (end === 'beyond-playable') {
		ask = lookupOtherAudio(q, batch);
	}
	let live = true;
	void ask?.then((answer) => {
		if (live) onAnswer(answer);
	});
	return () => {
		live = false;
	};
}
