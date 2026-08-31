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

	constructor(pageSize: number) {
		this.pageSize = pageSize;
	}

	/** 1-based strip page holding `episode`; page 1 for invalid input. */
	pageOf(episode: number): number {
		void episode;
		void this.pageSize;
		return 1;
	}

	/** Mount-time open at the page holding the current episode. */
	open(page: number): StripPagerFetch {
		void page;
		return { fetch: false };
	}

	/** User pagination: the pager arrows and the jump form. */
	userGoto(page: number, currentEpisode: number): StripPagerFetch {
		void page;
		void currentEpisode;
		return { fetch: false };
	}

	/** The URL's episode changed (prev/next buttons, auto-play). */
	episodeChanged(episode: number): StripPagerFetch {
		void episode;
		return { fetch: false };
	}

	/** A fetch resolved; true means apply its data. */
	completed(gen: number): boolean {
		void gen;
		return false;
	}

	/** A fetch failed; true means surface the error. */
	failed(gen: number): boolean {
		void gen;
		return false;
	}

	/** Is this generation still the newest? */
	isCurrent(gen: number): boolean {
		void gen;
		return false;
	}
}
