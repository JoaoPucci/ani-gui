/**
 * Maps backend errors to user-facing copy. The helpers, in order of
 * specificity:
 *
 *   • `describeError` (in ./describe-error, re-exported here) is the
 *     general one: a short localized sentence naming the cause by the
 *     error's `key`, or by its `kind` when the key is absent or
 *     unknown. Every surface that shows a failure without a copy of
 *     its own uses it.
 *   • `describeRateLimit`, `describeSourceDown` and
 *     `describeSourceAnswer` (in ./source-answer, re-exported here)
 *     are first-chance mappers for the typed
 *     provider answers (busy, down, any other status);
 *     describePlayFailure calls them before its own branches.
 *   • `describePlayFailure` picks the right user-facing message for
 *     a play-call failure — "no episode," "scraper unhappy,"
 *     "network trouble," etc.
 *   • `describeExternalLaunchFailure`, in ./external-launch-copy, covers
 *     "Open in external player" on top of `describePlayFailure`.
 *
 * None of them prints the payload's `detail`. The backend documents it
 * as free text for logs (ParseFailed carries serde's message, a URL that
 * failed validation, …), and a raw `kind` token or a thrown Error's
 * message is no better on screen.
 *
 * Extracted from the play page so the message branches can be
 * unit-tested instead of being threaded through Svelte effect
 * runtime.
 */

import { m } from '$lib/paraglide/messages';

/** Flatten a thrown value into the lowercase text `describePlayFailure`
 *  matches its branches against. Internal: it can carry the payload's
 *  detail, so it is for classification only and never shown. */
function classifierText(e: unknown): string {
	if (typeof e === 'object' && e !== null) {
		const obj = e as Record<string, unknown>;
		const kind = typeof obj.kind === 'string' ? obj.kind : null;
		const detail = typeof obj.detail === 'string' ? obj.detail : null;
		if (kind && detail) return `${kind}: ${detail}`.toLowerCase();
		if (kind) return kind.toLowerCase();
	}
	return String(e).toLowerCase();
}

export { describeError } from './describe-error';
import { describeError } from './describe-error';

export { describeRateLimit, describeSourceAnswer, describeSourceDown } from './source-answer';
import { describeRateLimit, describeSourceAnswer, describeSourceDown } from './source-answer';

/** What the availability cache says about the other audio of the
 *  show a play failed for: `true` when it has the show (and, where the
 *  cache carries a count, this episode), `false` when it is known to
 *  have nothing, `null` when nothing vouches either way — never
 *  probed, an answer the cache would not serve, or a lookup that
 *  failed. Learned from the cache only (./other-audio). */
export type OtherAudio = boolean | null;

/** What a surface tells `describePlayFailure` beyond the error. */
export interface PlayFailureContext {
	/** The other audio's known availability. Required, so no surface
	 *  can show the episode verdict without having answered it. */
	otherAudio: OtherAudio;
	/** The surface's own catalogue-miss phrasing, if it has one. */
	noResults?: () => string;
}

/** Whether a play failure is the episode verdict: the show was
 *  found and this episode has no stream in the chosen audio. The one
 *  failure whose copy depends on the other audio. */
export function isEpisodeUnavailable(e: unknown): boolean {
	return classifierText(e).includes('episode_unavailable');
}

/** User-facing copy for a play-call failure. The message branches
 *  match (in order): rate_limited / upstream 429 → busy source (with
 *  the upstream's own retry hint when it sent one); upstream 5xx →
 *  source down; upstream 404/410 → catalogue miss; other upstream →
 *  the source answered with an error; episode_unavailable → the show
 *  is there and this episode is not, suggesting the other audio only
 *  when `ctx.otherAudio` is true; stream_server_failed → the show
 *  is there and its video server did not serve the episode (the
 *  key's own sentence, shared with every other surface); no_results →
 *  catalogue miss; scraper → upstream unhappy; timeout → slow
 *  upstream; network / gate_refused → connection trouble; default →
 *  generic retry. */
export function describePlayFailure(e: unknown, ctx: PlayFailureContext): string {
	const rateLimited = describeRateLimit(e);
	if (rateLimited !== null) return rateLimited;
	const sourceDown = describeSourceDown(e);
	if (sourceDown !== null) return sourceDown;
	const answered = describeSourceAnswer(e);
	if (answered !== null) return answered;
	const raw = classifierText(e);
	if (isEpisodeUnavailable(e)) {
		// The show is in the catalogue; this episode has no stream in
		// the requested audio. The same copy on every surface — the
		// detail page's catalogue-miss phrasing is for a title the
		// catalogue lacks, which this is not. The other audio is
		// suggested only when the cache vouches for it: sent to an
		// audio known to have nothing, the user finds Play hidden
		// there with no reason given, and an unknown is no better.
		return ctx.otherAudio === true
			? m.play_play_failure_episode_unavailable_try_other_audio()
			: m.play_play_failure_episode_unavailable();
	}
	if (raw.includes('stream_server_failed')) {
		// The show is in the catalogue and its stream server did not
		// serve this episode — unreachable, timing out, or answering
		// nothing playable — past searches that answered. The key's own
		// sentence names the source's video server and the episode,
		// which is what every surface should say, so no play-surface
		// variant; and it must be read before the broader branches
		// below, whose kinds this one stands for.
		return describeError(e);
	}
	if (raw.includes('no_results')) {
		// The one deliberate per-surface difference: the detail page
		// phrases a catalogue miss definitively (it also gates the
		// Play CTA proactively); every other surface keeps the hedge.
		return (ctx.noResults ?? m.play_play_failure_no_results)();
	}
	if (raw.includes('scraper')) {
		return m.play_play_failure_scraper();
	}
	if (raw.includes('timeout')) {
		return m.play_play_failure_timeout();
	}
	if (raw.includes('network') || raw.includes('gate_refused')) {
		return m.play_play_failure_network();
	}
	return m.play_play_failure_generic();
}
