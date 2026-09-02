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
	/** Outstanding nudges per rendition. Budgets are independent:
	 *  interleaved failures must not leak budget across renditions,
	 *  and one rendition recovering clears only its own burst.
	 *  Failures without frag data stall the main rendition (the
	 *  common case — hls.js fatals do not always carry a frag). */
	private nudges = new Map<string, number>();
	private hasProgressed = false;

	/** A fatal error arrived; decide the response. A nudge counts
	 *  against ITS rendition's burst budget and asks for the toast
	 *  only when no stall was active — one "host is slow" notice per
	 *  trouble window, however many renditions join it. */
	failure(input: { err: StreamFailure; hasAutoRetried: boolean; rendition?: string }): StallAction {
		const rendition = input.rendition ?? 'main';
		const used = this.nudges.get(rendition) ?? 0;
		const response = decideStreamFailureResponse({
			...input,
			nudgesUsed: used,
			playbackProgressed: this.hasProgressed
		});
		if (response === 'nudge') {
			const anyActive = [...this.nudges.values()].some((n) => n > 0);
			this.nudges.set(rendition, used + 1);
			return { act: 'nudge', toast: !anyActive };
		}
		return { act: response };
	}

	/** A fragment landed: its own rendition's burst is over. A side
	 *  rendition proves nothing about a video stall, and arriving
	 *  video proves nothing about an audio stall. */
	fragmentLoaded(data: { frag?: { type?: string } }): void {
		const type = data.frag?.type;
		if (type !== undefined) this.nudges.delete(type);
	}

	/** Playback crossed the running threshold on the CURRENT stream:
	 *  the URL is proven, and stays proven for this stream's lifetime
	 *  — a viewer seeking back to the first second must not turn a
	 *  proven stream's stall into the disruptive re-resolve. Cleared
	 *  only by reset(). */
	progressed(): void {
		this.hasProgressed = true;
	}

	/** A recovery replaced the session, or the user switched
	 *  episodes: the next stream gets fresh budgets and must prove
	 *  itself again. */
	reset(): void {
		this.nudges.clear();
		this.hasProgressed = false;
	}
}

/** The shared machine, module-level like the singleton video whose
 *  stream it guards: the hls callbacks survive a route unmount (PiP),
 *  so burst and progress state must not fork per component mount. */
export const stallMachine = new HlsStallMachine();
