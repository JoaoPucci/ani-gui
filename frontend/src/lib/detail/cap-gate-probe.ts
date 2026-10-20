import { probeScope, type ProbeRun } from '$lib/availability/probe-scope';
import type { AvailabilityResponse, VersionInfo } from '$lib/api';
import { beyondPlayable } from './episode-caps';
import { availabilityVerdict, refusedByName } from '$lib/availability/verdict';

/**
 * Re-asking availability when a user clicks a cap-gated episode tile.
 *
 * A tile is cap-gated when the anime database says the episode aired
 * but it sits above the episode count the provider reported — the
 * catalog-lag case `beyondPlayable` describes. That count is a
 * snapshot, cached for 24 hours on an ongoing show, and the provider adds
 * episodes inside that window. So the tile can stay dead for most of
 * a day after the episode became streamable, and until now the click
 * returned early: no request, no message, nothing to distinguish it
 * from a broken control.
 *
 * The re-ask must therefore set `bypass_cache`. Without it the lookup
 * answers from the very row being questioned, confirms the tile's own
 * claim, and reaches nobody — which is what the first version of this
 * did. The interactive lane matters too: the scraper gate paces and
 * refuses background traffic but never a request a user is waiting
 * on.
 *
 * ONE LOOKUP PER SHOW. "How many episodes do you have?" does not vary
 * by episode, so three dimmed tiles clicked in a row are the same
 * question three times at a rate-limited site. The first click sends
 * it, later clicks join, and every waiting tile is judged against the
 * single answer. Sharing lasts only while the request is in flight —
 * a click afterwards deserves a current answer rather than a replay.
 *
 * Outcomes, and why each is what it is:
 *
 *   - The fresh count reaches the episode → play it. The click
 *     already expressed the intent; the stale count was the obstacle.
 *   - It still falls short, the lookup produced no count, or it
 *     failed → say so. Silence is indistinguishable from the dead
 *     tile this exists to replace.
 */
/** What the provider said, whole. The re-ask replaces the backend's
 *  entire cached row, so the answer is a row rather than a number. */
export interface CapGateAnswer {
	/** Highest integer episode, or null when there is no number. */
	count: number | null;
	/** The count came from the search hit rather than the per-show
	 *  fetch — it counts half-episodes as whole ones and can read one
	 *  high. Also means `extraEpisodes` is empty because the fetch
	 *  that would have listed them failed. */
	approximate: boolean;
	/** Whether the catalogue has the show at all. */
	available: boolean;
	/** Non-integer episode tags — recaps and specials, e.g. "1061.5". */
	extraEpisodes: string[];
	/** The show's versions, when its pick has several; none otherwise —
	 *  a refused or delisted show among them. */
	versions: VersionInfo[];
	/** True when `available: false` is the provider refusing the title
	 *  by name. */
	refused?: boolean;
}

/**
 * The parts of a fresh answer a route may write to its own state.
 *
 * The routes were reading one field out of a whole replaced row, so
 * a delisted show stayed enabled and a strip kept its mount-time
 * specials. This is what survives validation — same context, and
 * confirmed where confirmation matters.
 */
export interface CapGateRefresh {
	/** A search that could not be completed comes back as a failure,
	 *  never as a false. A false is the provider's answer: the show
	 *  not listed, or listed under nothing named for it — the refusal
	 *  by name, an inference from how the catalogue names its listings
	 *  that the backend does not persist and asks again on the next
	 *  look. */
	available: boolean;
	/** True when the false is a refusal by name: the title is in the
	 *  catalogue under nothing named for it, in any audio. */
	refused?: boolean;
	/** The confirmed cap, or null when the answer did not establish one
	 *  — a count that can read one high must not replace a real cap.
	 *
	 *  A confirmed answer with no number is NOT null here but zero.
	 *  Null is the routes' word for "no cap known", which
	 *  `beyondPlayable` reads as unbounded; a delisted show, or one
	 *  whose whole episode list is non-integer tags, means the
	 *  opposite. */
	count: number | null;
	/** Null when the answer was unconfirmed. The empty list an
	 *  unconfirmed answer carries means "could not look", not "there
	 *  are none", and writing it would delete specials that exist. */
	extraEpisodes: string[] | null;
	/** The show's versions, null when the answer was unconfirmed for
	 *  the same reason: they come from the fetch the count does, and
	 *  an empty list from an answer that could not look would take a
	 *  choice away that exists. */
	versions: VersionInfo[] | null;
}

/**
 * The answer a route hands the controller, read from the backend's
 * wire shape: the whole row — the controller decides which parts of a
 * fresh answer are safe to write back. An unsettled answer is no
 * answer: the probe reached the provider and came back without a
 * verdict, and its `available: false` means nothing, so it is the
 * controller's null — "nothing was established" — the same path the
 * request's failure takes. Both routes read answers through this, so
 * neither can take that false for the show being gone. A refusal by
 * name is the exception: the catalogue lists nothing named for the
 * show, and the answer is the show unavailable
 * (`$lib/availability/verdict`).
 */
export function capGateAnswerFrom(r: AvailabilityResponse): CapGateAnswer | null {
	const available = availabilityVerdict(r);
	if (available === null) return null;
	return {
		count: r.episode_count,
		approximate: r.episode_count_approximate === true,
		available,
		extraEpisodes: r.extra_episodes ?? [],
		versions: r.versions ?? [],
		// Present only when true, as every answer before refusals reads.
		...(refusedByName(r) ? { refused: true } : {})
	};
}

export interface CapGateProbeDeps {
	/** Ask the provider about THIS SHOW, skipping the cached row. Answers
	 *  with the whole fresh row; null when it could not answer at all.
	 *  Takes no episode: the question is show-level. `signal` is the
	 *  recheck's own run, which `supersede` ends. */
	probe: (signal?: AbortSignal) => Promise<CapGateAnswer | null>;
	/** What the answer will be about — read when the click goes out and
	 *  again when it lands. Everything that changes the meaning of a
	 *  count belongs in here: the show, because both routes reuse one
	 *  component across titles, and the audio mode, because the provider
	 *  catalogues sub and dub separately and settings arrive after the
	 *  page does. The pages also put in whatever means nobody is left
	 *  to act on it: the page gone, or a navigation started since that
	 *  leaves it. */
	currentContext: () => string;
	/** The fresh count reaches the episode — apply the refresh and
	 *  play. `count` is the same number as `refresh.count`, passed
	 *  separately so the play path does not re-narrow a nullable. */
	onCleared: (episode: number, count: number, refresh: CapGateRefresh) => void;
	/** The catalogue answered, and the episode is still not in it. The
	 *  refresh still matters — the show may be gone, the cap may have
	 *  shrunk, the specials may have changed. */
	onStillGated: (episode: number, refresh: CapGateRefresh) => void;
	/** Nothing was established about whether this episode is there —
	 *  the lookup could not be made, or it came back unconfirmed. A
	 *  different sentence from still-gated: saying "not in the
	 *  catalogue" here claims the very thing that went unanswered.
	 *
	 *  `refresh` is null when there was no answer at all, and carries
	 *  what an unconfirmed one did establish otherwise — the search
	 *  succeeded even when the per-show fetch did not. */
	onFailed: (episode: number, refresh: CapGateRefresh | null) => void;
	/** The answer arrived about a show the user has left. Nothing can
	 *  be said and nothing can be played, but the page has to be let
	 *  go of — it is blocked waiting on this. */
	onSuperseded: (episode: number) => void;
}

/**
 * Which parts of an answer a route may write. Four fields, three
 * different reasons to trust them.
 *
 * The verdict always. It comes from the search, which either found
 * the show or did not; an incomplete search is reported upstream as
 * a failure, so a `false` here was established rather than assumed.
 *
 * The extras whenever the per-show fetch answered. That fetch is
 * what lists them, so `approximate` — which means precisely that it
 * did not answer — is the only thing that makes the empty list a
 * "could not look" rather than a "there are none". A delisted show
 * genuinely has none, and a show whose episodes are all non-integer
 * has a real list alongside a null count. The show's versions are the
 * same fetch's, and go with the extras.
 *
 * The count whenever it is confirmed — including when there is no
 * number. A count that can read one high must not replace a real
 * cap, so an unconfirmed one is withheld. But a CONFIRMED answer
 * without a number is itself a fact: The provider has the show delisted,
 * or its whole episode list is non-integer tags. That is a cap of
 * zero, not an absent cap — null is the routes' word for "unknown",
 * which `beyondPlayable` reads as unbounded.
 */
function refreshFrom(answer: CapGateAnswer): CapGateRefresh {
	return {
		available: answer.available,
		...(answer.refused === true ? { refused: true } : {}),
		count: answer.approximate ? null : (answer.count ?? 0),
		extraEpisodes: answer.approximate ? null : answer.extraEpisodes,
		versions: answer.approximate ? null : answer.versions
	};
}

export function createCapGateProbe(deps: CapGateProbeDeps): {
	request: (episode: number) => void;
	isProbing: (episode: number) => boolean;
	/** The page's context has moved on (another mode, another show on
	 *  the same route): stop the recheck's probe, whose answer would
	 *  be discarded anyway. Its tiles are let go as superseded. */
	supersede: () => void;
} {
	/** The shared in-flight lookup, or null when none is out. */
	let pending: Promise<CapGateAnswer | null> | null = null;
	/** The run the in-flight lookup probes under. */
	let run: ProbeRun | null = null;
	/** Tiles waiting on it — per episode, because the spinner belongs
	 *  to the tile the user pressed rather than to the whole strip. */
	const waiting = new Set<number>();

	return {
		isProbing: (episode) => waiting.has(episode),
		supersede: () => {
			run?.end();
			run = null;
			// A click from here on asks afresh rather than joining the
			// stopped lookup.
			pending = null;
		},
		request: (episode) => {
			if (waiting.has(episode)) return;
			waiting.add(episode);
			// What this question is about. Compared again when the answer
			// lands: an episode number means a different episode on a
			// different strip, and a sub count says nothing about dub,
			// so applying an answer across either change plays the wrong
			// thing.
			const asked = deps.currentContext();
			// Cleared as soon as the request settles, so a click after
			// this one gets a current answer instead of joining a
			// lookup that has already finished.
			if (!pending) {
				const own = probeScope.begin();
				run = own;
				const asking: Promise<CapGateAnswer | null> = deps.probe(own.signal).finally(() => {
					// Only its own: a lookup superseded and replaced must
					// not clear the one that replaced it.
					if (pending === asking) pending = null;
					if (run === own) run = null;
				});
				pending = asking;
			}
			void pending
				.then((answer) => {
					if (deps.currentContext() !== asked) return deps.onSuperseded(episode);
					if (answer == null) return deps.onFailed(episode, null);
					const refresh = refreshFrom(answer);
					// An unconfirmed answer means the per-show fetch did
					// not respond — and that fetch is the thing that would
					// have said whether this episode is there. So it is
					// not a catalogue verdict, however low the search-hit
					// count reads. It still carries what the SEARCH
					// established.
					//
					// The null check is redundant behind `approximate` —
					// nothing else produces a null cap — but it is what
					// narrows the type for the comparisons below, and it
					// keeps the two from drifting apart.
					if (answer.approximate || refresh.count == null) {
						return deps.onFailed(episode, refresh);
					}
					// `beyondPlayable` decides the rest, rather than a
					// fresh comparison: the rule that dimmed the tile has
					// to be the rule that un-dims it, half-episode
					// floor-compare included.
					//
					// The refresh goes back either way. A confirmed count
					// SHORTER than what the page is showing is news:
					// The provider pulls episodes and corrects metadata, and
					// until the strip hears about it every tile between
					// the two caps stays enabled on a number that is no
					// longer true.
					if (beyondPlayable(episode, refresh.count)) {
						return deps.onStillGated(episode, refresh);
					}
					deps.onCleared(episode, refresh.count, refresh);
				})
				// Same guard on the failure path: an error toast about a
				// title the user has already left is noise attached to
				// something they are no longer doing.
				.catch(() =>
					deps.currentContext() !== asked
						? deps.onSuperseded(episode)
						: deps.onFailed(episode, null)
				)
				// Released on every path. A rejection that left the tile
				// marked would swallow its later clicks as duplicates and
				// wedge it shut for good.
				.finally(() => waiting.delete(episode));
		}
	};
}
