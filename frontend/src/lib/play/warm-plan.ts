/**
 * Which episodes a page-mount warm should resolve.
 *
 * With resolution caching opted in (Config::cache_resolutions), the
 * wide fan-out earns its cost: warms mostly land as ~0.5s cache hits
 * on a revisit, and every visible tile click becomes instant. With
 * caching off — the default — every warm is a full multi-second
 * provider walk, so the plan narrows to the single episode the user
 * is most likely to play next: the strip's current+1 on the play
 * page (what keeps auto-play seamless), the Play button's target on
 * the detail page. Candidates and the narrow target are validated by
 * the caller (aired, within the playable cap); this function only
 * chooses between the two shapes.
 */
export interface WarmPlanInput {
	/** The user's Config::cache_resolutions. */
	cacheResolutions: boolean;
	/** Wide fan-out: every warmable episode the page would resolve. */
	candidates: number[];
	/** Narrow target: the one episode worth a full walk, or null
	 *  when there is none (last episode, unaired next, no cap). */
	next: number | null;
}

export function planWarm(input: WarmPlanInput): number[] {
	return input.candidates;
}
