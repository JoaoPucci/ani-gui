/**
 * What the search page keeps when the reader leaves it, so going back
 * to it restores what they left: the query, the order, the type
 * filters, the results already loaded, and which card was at the top of
 * the screen.
 *
 * SvelteKit's snapshots carry it (https://svelte.dev/docs/kit/snapshots):
 * the page's `snapshot.capture` runs just before it is left and the
 * value is kept with that history entry; on a back or forward press
 * onto the entry, `snapshot.restore` gets it back. SvelteKit also
 * writes the value to sessionStorage, so it must be plain JSON and it
 * may come back from an older version of the app: `readSnapshot`
 * checks it before anything trusts it.
 */

import type { KitsuAnimeRef, SearchOrder } from '$lib/api';
import { compactHit } from './saved-hit';

export { readSnapshot } from './snapshot-read';

/** How long saved results may be shown again without asking for them. */
export const FRESH_FOR_MS = 60 * 60 * 1000;

/** The most results a snapshot carries; past that it keeps only where
 *  the reader was, and the search is run again on the way back. */
export const MAX_SAVED_HITS = 400;

/** Where the runner's search stood when its last page settled. */
export interface SearchCheckpoint {
	/** The audio the availability filter judged the results for. */
	mode: 'sub' | 'dub';
	/** When the search's first page was fetched, so no result shown is
	 *  older to this page than that. Absent from a checkpoint that cannot
	 *  tell, which is never reused. */
	fetchedAt?: number;
	/** The results shown, in order. */
	hits: KitsuAnimeRef[];
	/** Every id a page has carried, shown or not. */
	seen: string[];
	/** The offset of the next page; `null` once there is none. */
	next: number | null;
	emptyStreak: number;
	/** Pages loaded. */
	pages: number;
	phase: 'idle' | 'end' | 'held';
}

/** A card on screen, in viewport coordinates. */
export interface CardPosition {
	id: string;
	top: number;
	bottom: number;
}

/** The card at the top of the screen, and how far below the top of the
 *  viewport it sat. */
export interface ScrollAnchor {
	id: string;
	top: number;
}

export interface SearchSnapshot {
	v: 1;
	/** The query whose results were on screen; empty for none. */
	q: string;
	order: SearchOrder;
	subtypes: string[];
	/** When the reader left the page. Not how fresh the results are:
	 *  that is `results.fetchedAt`, which `canReuse` reads. */
	capturedAt: number;
	/** Pages the search had loaded. */
	pages: number;
	anchor: ScrollAnchor | null;
	/** `null` when the results cannot come back as they were: none had
	 *  settled, the search had failed, or there were too many. */
	results: SearchCheckpoint | null;
}

export interface CaptureInput {
	q: string;
	order: SearchOrder;
	subtypes: string[];
	checkpoint: SearchCheckpoint | null;
	cards: CardPosition[];
	now: number;
}

/** The snapshot the search page saves as the reader leaves it. */
export function captureSearch(input: CaptureInput): SearchSnapshot {
	const cp = input.checkpoint;
	const keep = cp !== null && cp.hits.length <= MAX_SAVED_HITS;
	return {
		v: 1,
		q: input.q,
		order: input.order,
		subtypes: [...input.subtypes],
		capturedAt: input.now,
		pages: cp?.pages ?? 0,
		anchor: pickAnchor(input.cards),
		results: keep ? { ...cp, hits: cp.hits.map(compactHit), seen: [...cp.seen] } : null
	};
}

/** The first card still on screen: the one the reader was looking at
 *  the top of. */
export function pickAnchor(cards: CardPosition[]): ScrollAnchor | null {
	const card = cards.find((c) => c.bottom > 0);
	return card ? { id: card.id, top: card.top } : null;
}

/**
 * Whether saved results may be shown again as they are, judged by
 * when they were fetched (not when the page was left, which would let
 * a search left open for hours, or one taken back and forth, renew
 * itself without asking). Only on a back
 * or forward press, not a reload (which asks for the page afresh);
 * only for the audio setting they were judged for, since the
 * availability filter chose them by it; and only within
 * {@link FRESH_FOR_MS}, the shortest time the backend trusts an
 * availability verdict for (`FLOOR_SECS` in
 * `backend/src/commands/availability_ttl.rs`), past which a show may
 * have gone from the provider or arrived on it.
 */
export function canReuse(
	saved: SearchSnapshot,
	at: { now: number; mode: 'sub' | 'dub'; reload: boolean }
): boolean {
	const results = saved.results;
	if (at.reload || results?.mode !== at.mode || results.fetchedAt === undefined) return false;
	const age = at.now - results.fetchedAt;
	return age >= 0 && age < FRESH_FOR_MS;
}

/** What a search run again on the way back aims for: the card that was
 *  at the top of the screen, within the pages it had loaded. */
export interface ResumeTarget {
	/** `null` when no card was on screen. */
	id: string | null;
	pages: number;
}

/**
 * What a search run again on the way back does once a page has settled:
 * it has `found` the card, loads one `more` page, has run out of pages
 * to look in (`gone`), or waits while the runner is already loading the
 * next page by itself (a page that showed nothing). A search that is
 * not aiming for a card (`target` null) always waits.
 */
export function resumeStep(
	target: ResumeTarget | null,
	at: { shown: readonly KitsuAnimeRef[]; pages: number; phase: string }
): 'more' | 'found' | 'gone' | 'wait' {
	if (target === null) return 'wait';
	if (target.id === null) return 'gone';
	if (at.shown.some((h) => h.id === target.id)) return 'found';
	if (at.phase === 'loading-more') return 'wait';
	return at.phase === 'idle' && at.pages < target.pages ? 'more' : 'gone';
}
