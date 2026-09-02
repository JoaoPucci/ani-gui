/**
 * The HLS stall lifecycle: burst counting, the one-toast rule, and
 * the burst boundary, composed over the failure ladder in
 * stale-stream.ts. The play page's hls handlers are event adapters —
 * they hand the machine what hls.js reported and perform the action
 * it returns (startLoad, the recovery flow, the error overlay);
 * every transition lives here under unit test (AGENTS.md §2).
 *
 * The burst boundary is a landed fragment of the MAIN rendition
 * only. Manifests split video, audio and subtitle renditions, and a
 * side rendition landing proves nothing about the stalled video —
 * an unconditional reset would refill the budget between video
 * failures and the burst could never escalate to the fresh-link
 * recovery. Playback progress is not the signal either: a stalled
 * network with buffered media keeps the clock moving.
 */

import { decideStreamFailureResponse, type StreamFailure } from '$lib/play/stale-stream';

export type StallAction =
	| { act: 'nudge'; toast: boolean }
	| { act: 'recover' }
	| { act: 'surface' };

export class HlsStallMachine {
	/** A fatal error arrived; decide the response. A nudge counts
	 *  against the burst budget and asks for the toast only when it
	 *  opens the burst. */
	failure(input: {
		err: StreamFailure;
		hasAutoRetried: boolean;
		playbackProgressed: boolean;
	}): StallAction {
		void input;
		void decideStreamFailureResponse;
		return { act: 'surface' };
	}

	/** A fragment landed. Only the main rendition ends the burst. */
	fragmentLoaded(data: { frag?: { type?: string } }): void {
		void data;
	}

	/** A recovery replaced the session, or the user switched
	 *  episodes: the next stream gets a fresh budget. */
	reset(): void {}
}
