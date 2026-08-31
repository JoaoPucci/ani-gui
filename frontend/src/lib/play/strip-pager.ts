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

export class StripPager {
	private readonly pageSize: number;
	private gen = 0;
	private requestedPage: number | null = null;
	private renderedPage = 1;
	private tracking = true;

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

	private issue(page: number): StripPagerFetch {
		this.requestedPage = page;
		this.gen += 1;
		return { fetch: true, page, gen: this.gen };
	}

	/** Mount-time open at the page holding the current episode. */
	open(page: number): StripPagerFetch {
		if (!Number.isFinite(page) || page < 1) return { fetch: false };
		return this.issue(page);
	}

	/** User pagination: the pager arrows and the jump form. Moving to
	 *  the current episode's page keeps the strip tracking playback;
	 *  moving anywhere else is browsing, and follows stop until
	 *  playback enters the browsed page. */
	userGoto(page: number, currentEpisode: number): StripPagerFetch {
		if (!Number.isFinite(page) || page < 1) return { fetch: false };
		if (page === this.current()) return { fetch: false };
		this.tracking = page === this.pageOf(currentEpisode);
		return this.issue(page);
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
			return { fetch: false };
		}
		if (!this.tracking) return { fetch: false };
		return this.issue(target);
	}

	/** A fetch resolved. True means apply its data — it is the newest
	 *  generation; the pager records its page as rendered. A stale
	 *  completion returns false and must be discarded. */
	completed(gen: number): boolean {
		if (gen !== this.gen) return false;
		this.renderedPage = this.requestedPage ?? this.renderedPage;
		return true;
	}

	/** A fetch failed. True means surface the error (newest
	 *  generation); the requested page reverts to the rendered one so
	 *  a later attempt at the failed page is not swallowed by the
	 *  already-there guard. `tracking` deliberately survives: a
	 *  transient failure must not turn a following strip into a
	 *  browsing one, or the next episode change would decline the
	 *  retry and strand the strip. */
	failed(gen: number): boolean {
		if (gen !== this.gen) return false;
		this.requestedPage = this.renderedPage;
		return true;
	}

	/** Is this generation still the newest? The component keys its
	 *  loading flag off this so a superseded fetch's settling cannot
	 *  clear the flag out from under its superseder. */
	isCurrent(gen: number): boolean {
		return gen === this.gen;
	}
}
