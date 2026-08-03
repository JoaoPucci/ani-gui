/**
 * The page-load availability lookup, as a sequence rather than as
 * forty lines inside a component.
 *
 * Both routes run the same one: ask allmanga about this show in this
 * mode, hand whatever comes back to the writeback so it can decide
 * which parts are still this lookup's to write, and report the
 * question answered — once, whichever way it went. A lookup that has
 * been cancelled reports nothing at all, because the page it was for
 * is not the page that would hear it.
 *
 * The component keeps the two reactive reads (`detail`, the mode) and
 * hands them here. Everything with an ordering rule in it lives on
 * this side, where it can be driven directly.
 */

import type { AvailabilityArgs, AvailabilityResponse } from '$lib/api';
import type { AvailabilityAnswer, AvailabilityPatch } from './availability-writeback';

/** The show being asked about, in the shape the request wants. */
export interface AvailabilitySubject {
	title: string;
	altTitles: string[];
	episodeCount?: number;
	year?: number;
	subtype?: string | null;
	kitsuId: string;
	status?: string;
}

export interface AvailabilityLookupDeps {
	check: (args: AvailabilityArgs) => Promise<AvailabilityResponse>;
	/** Opens the writeback ticket. Called before the request goes out,
	 *  so what it captures describes the moment the question was asked. */
	begin: () => (answer: AvailabilityAnswer) => AvailabilityPatch;
	apply: (patch: AvailabilityPatch) => void;
	/** False while the question is open, true once it is answered —
	 *  including when it failed, since "we asked and could not tell" is
	 *  an answer the page acts on. */
	setResolved: (resolved: boolean) => void;
}

/**
 * Start one lookup. Returns the canceller: after it runs, nothing
 * further is written and the resolved flag is left where the next
 * lookup will set it.
 */
export function startAvailabilityLookup(
	subject: AvailabilitySubject,
	mode: 'sub' | 'dub',
	deps: AvailabilityLookupDeps
): () => void {
	let cancelled = false;
	deps.setResolved(false);
	const settle = deps.begin();
	void deps
		.check({
			title: subject.title,
			mode,
			alt_titles: subject.altTitles,
			episode_count: subject.episodeCount,
			year: subject.year,
			kitsu_id: subject.kitsuId,
			status: subject.status
		})
		.then((r) => {
			if (cancelled) return;
			// Whatever a re-ask has since established is the re-ask's —
			// it is newer and it bypassed the cache. Anything it left
			// unanswered is still this lookup's to fill.
			deps.apply(
				settle({
					available: r.available,
					count: r.episode_count,
					extraEpisodes: r.extra_episodes
				})
			);
		})
		.catch(() => {
			// Nothing established. The page keeps what it had, and the
			// lazy failure path in the click handler still surfaces it.
		})
		.finally(() => {
			if (!cancelled) deps.setResolved(true);
		});
	return () => {
		cancelled = true;
	};
}
