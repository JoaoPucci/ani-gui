/**
 * State machine for the play page's episode strip pagination.
 *
 * The strip shows a fixed page of tiles; episode switches replace the
 * URL on the same route, so the page never remounts and the strip has
 * to follow playback on its own. Fetches are asynchronous and can
 * race, so the machine owns four pieces of state the component must
 * not improvise over:
 *
 *   • `tracking`  — whether the strip is following playback. The user
 *     paginating away flips it off; it re-latches when playback
 *     enters the page they are looking at. It survives a failed
 *     fetch: a follow that failed transiently was still tracking, and
 *     forgetting that would strand the strip until manual pagination.
 *   • `requestedPage` — the page the newest fetch is bringing in,
 *     which the rendered page lags while that fetch is in flight.
 *     Decisions compare against this, not the rendered page, so
 *     navigation reversing mid-flight arms a superseding fetch.
 *   • `gen` — monotonic fetch generation. Only the newest generation
 *     may apply its completion; a slow stale response cannot
 *     overwrite the page the strip already moved to. Same guard
 *     shape as the scrubber's drag generation.
 *   • `renderedPage` — the page whose data the component last
 *     applied; the truth `requestedPage` reverts to on failure.
 *
 * Pure TypeScript, no Svelte runtime: the component is a thin
 * adapter that calls `open` / `userGoto` / `episodeChanged` and runs
 * the fetch each returned instruction asks for, reporting the
 * outcome back through `completed` / `failed`.
 */
export type StripPagerFetch = { fetch: true; page: number; gen: number } | { fetch: false };

/** Outcome of a failed fetch: whether the error should be surfaced,
 *  and a follow retry to run when the failure recovered an episode
 *  change the fetch had absorbed (the retry supersedes the error). */
export type StripPagerFailure = { surface: boolean; retry: StripPagerFetch };

export class StripPager {
	private readonly pageSize: number;
	private gen = 0;
	private requestedPage: number | null = null;
	private renderedPage = 1;
	private tracking = true;
	/** `tracking` as it stood before the newest request was issued —
	 *  what a failure restores, so a failed request reverts the whole
	 *  machine as if it never happened. */
	private trackingBeforeRequest = true;
	/** Playback's latest position that an in-flight request kept the
	 *  strip from reflecting — a change absorbed by a fetch already on
	 *  its way to its page, or one discarded while a browse was
	 *  pending. Recovery state: satisfied by a completion, superseded
	 *  by an issued follow, and consumed by the one replay a failure
	 *  hands back — the URL already sits at that episode, so no
	 *  further event would re-fire the follow. The reverted state
	 *  decides whether the replay is actionable. */
	private unreflectedEpisode: number | null = null;

	constructor(pageSize: number) {
		this.pageSize = pageSize;
	}

	/** 1-based strip page holding `episode`; page 1 for invalid input. */
	pageOf(episode: number): number {
		if (!Number.isFinite(this.pageSize) || this.pageSize < 1) return 1;
		if (!Number.isFinite(episode) || episode < 1) return 1;
		return Math.ceil(episode / this.pageSize);
	}

	/** Page decisions compare against: requested while a fetch is in
	 *  flight (or after one settled), rendered before the first. */
	private current(): number {
		return this.requestedPage ?? this.renderedPage;
	}

	private issue(page: number, tracking: boolean): StripPagerFetch {
		this.trackingBeforeRequest = this.tracking;
		this.tracking = tracking;
		this.requestedPage = page;
		this.gen += 1;
		return { fetch: true, page, gen: this.gen };
	}

	/** Mount-time open at the page holding the current episode. */
	open(page: number): StripPagerFetch {
		if (!Number.isFinite(page) || page < 1) return { fetch: false };
		return this.issue(page, true);
	}

	/** User pagination: the pager arrows and the jump form. Moving to
	 *  the current episode's page keeps the strip tracking playback;
	 *  moving anywhere else is browsing, and follows stop until
	 *  playback enters the browsed page. */
	userGoto(page: number, currentEpisode: number): StripPagerFetch {
		if (!Number.isFinite(page) || page < 1) return { fetch: false };
		if (page === this.current()) return { fetch: false };
		return this.issue(page, page === this.pageOf(currentEpisode));
	}

	/** The URL's episode changed (prev/next buttons, auto-play). */
	episodeChanged(episode: number): StripPagerFetch {
		if (!Number.isFinite(episode) || episode < 1) return { fetch: false };
		const target = this.pageOf(episode);
		if (target === this.current()) {
			// Playback entered the page the strip already shows (or is
			// fetching) — including a browsed page: the strip re-latches
			// onto playback the moment they meet.
			this.tracking = true;
			// A fetch still on its way to this page absorbs the change;
			// remember the episode so the fetch failing can hand back
			// the follow this change would otherwise have issued. With
			// nothing pending the displayed page already reflects
			// playback: any earlier ignored change is satisfied, and
			// replaying it later would move the strip away from the
			// playing episode.
			this.unreflectedEpisode =
				this.requestedPage !== null && this.requestedPage !== this.renderedPage ? episode : null;
			return { fetch: false };
		}
		if (!this.tracking) {
			// A browsing strip ignores playback — but remember where it
			// went: a pending browse failing reverts to a state that may
			// track playback again, and the URL effect for this episode
			// will not re-fire. (Harmless for a settled browsing strip:
			// a failure reverting to it replays into the same decline.)
			this.unreflectedEpisode = episode;
			return { fetch: false };
		}
		// This follow embodies playback's newest position: an older
		// unreflected change is superseded, not queued behind it — its
		// replay would drag the strip to a page playback already left.
		this.unreflectedEpisode = null;
		return this.issue(target, true);
	}

	/** A fetch resolved. True means apply its data — it is the newest
	 *  generation; the pager records its page as rendered. A stale
	 *  completion returns false and must be discarded. */
	completed(gen: number): boolean {
		if (this.requestedPage === null || gen !== this.gen) return false;
		this.renderedPage = this.requestedPage;
		this.unreflectedEpisode = null;
		return true;
	}

	/** A fetch failed. True means surface the error (newest
	 *  generation). The machine reverts to its pre-request state, as
	 *  if the failed request never happened: the requested page back
	 *  to the rendered one so a later attempt at the failed page is
	 *  not swallowed by the already-there guard, and `tracking` back
	 *  to what held before the request. A following strip whose
	 *  follow failed transiently keeps following (the next episode
	 *  change retries), and a failed browse does not pin
	 *  browsed-away intent to a page that never rendered. */
	failed(gen: number): StripPagerFailure {
		if (this.requestedPage === null || gen !== this.gen)
			return { surface: false, retry: { fetch: false } };
		this.requestedPage = this.renderedPage;
		this.tracking = this.trackingBeforeRequest;
		const unreflected = this.unreflectedEpisode;
		this.unreflectedEpisode = null;
		if (unreflected !== null) {
			// Replay the unreflected change through the ordinary
			// decision; with the machine reverted it either issues the
			// follow the change would have issued, or correctly
			// declines (e.g. the revert restored a browsing strip).
			const retry = this.episodeChanged(unreflected);
			if (retry.fetch) return { surface: false, retry };
		}
		return { surface: true, retry: { fetch: false } };
	}

	/** Is this generation still the newest? The component keys its
	 *  loading flag off this so a superseded fetch's settling cannot
	 *  clear the flag out from under its superseder. */
	isCurrent(gen: number): boolean {
		return gen === this.gen;
	}
}
