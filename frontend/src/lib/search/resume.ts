/**
 * The search runner's bookkeeping for going back to the search page
 * (`run-search.ts`, `snapshot.ts`): where a search stands once a page
 * settles, and what a saved search brings back.
 */

import type { KitsuAnimeRef } from '$lib/api';
import type { AfterPage } from './paging';
import {
	canReuse,
	type ResumeTarget,
	type ScrollAnchor,
	type SearchCheckpoint,
	type SearchSnapshot
} from './snapshot';

export type Settled = Omit<SearchCheckpoint, 'hits'>;

/** Where a search stands once a page has settled with `r`. A page
 *  loading by itself after an empty one leaves the search idle. */
export function checkpointAt(
	s: { mode: 'sub' | 'dub'; seen: Set<string>; next: number | null; loaded: number },
	r: AfterPage
): Settled {
	const kind = r.phase.kind;
	return {
		mode: s.mode,
		seen: [...s.seen],
		next: s.next,
		emptyStreak: r.emptyStreak,
		pages: s.loaded,
		phase: kind === 'end' || kind === 'held' ? kind : 'idle'
	};
}

/** What a saved search run again aims for: the card at the top of the
 *  screen, within the pages the reader had (at least the first). */
export function resumeTarget(saved: SearchSnapshot): ResumeTarget {
	return { id: saved.anchor?.id ?? null, pages: Math.max(1, saved.pages) };
}

/** Whether the anchored card is among `hits`. */
export function anchorShown(anchor: ScrollAnchor | null, hits: readonly KitsuAnimeRef[]): boolean {
	return anchor !== null && hits.some((h) => h.id === anchor.id);
}

/** The search state a kept checkpoint brings back. */
export function restoredState(kept: SearchCheckpoint) {
	const { hits, ...settled } = kept;
	return {
		pages: [hits],
		seen: new Set(kept.seen),
		next: kept.next,
		emptyStreak: kept.emptyStreak,
		loaded: kept.pages,
		settled
	};
}

/** The checkpoint of a search: where it stood when its last page
 *  settled, with the results it shows; `null` before then. */
export function checkpointOf(
	s: { settled: Settled | null; pages: KitsuAnimeRef[][] } | null
): SearchCheckpoint | null {
	return s?.settled ? { ...s.settled, hits: s.pages.flat() } : null;
}

/** How a saved search comes back: its `kept` results as they were,
 *  when `canReuse` allows, or run again aiming for `target`. */
export function resumePlan(
	saved: SearchSnapshot,
	at: { now: number; reload: boolean; mode: 'sub' | 'dub' }
): { kept: SearchCheckpoint } | { target: ResumeTarget } {
	return saved.results && canReuse(saved, at)
		? { kept: saved.results }
		: { target: resumeTarget(saved) };
}
