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

/** How long saved results may be shown again without asking for them. */
export const FRESH_FOR_MS = 60 * 60 * 1000;

/** The most results a snapshot carries; past that it keeps only where
 *  the reader was, and the search is run again on the way back. */
export const MAX_SAVED_HITS = 400;

/** Where the runner's search stood when its last page settled. */
export interface SearchCheckpoint {
	/** The audio the availability filter judged the results for. */
	mode: 'sub' | 'dub';
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

/** Declared ahead of its rules. */
export function captureSearch(input: CaptureInput): SearchSnapshot {
	return {
		v: 1,
		q: input.q,
		order: input.order,
		subtypes: [],
		capturedAt: input.now,
		pages: 0,
		anchor: null,
		results: null
	};
}

/** Declared ahead of its rules. */
export function pickAnchor(cards: CardPosition[]): ScrollAnchor | null {
	void cards;
	return null;
}

/** Declared ahead of its rules. */
export function readSnapshot(raw: unknown): SearchSnapshot | null {
	void raw;
	return null;
}

/** Declared ahead of its rules. */
export function canReuse(
	saved: SearchSnapshot,
	at: { now: number; mode: 'sub' | 'dub'; reload: boolean }
): boolean {
	void saved;
	void at;
	return false;
}

/** What a search run again on the way back aims for: the card that was
 *  at the top of the screen, within the pages it had loaded. */
export interface ResumeTarget {
	/** `null` when no card was on screen. */
	id: string | null;
	pages: number;
}

/** Declared ahead of its rules. */
export function resumeStep(
	target: ResumeTarget,
	at: { shown: readonly KitsuAnimeRef[]; pages: number; phase: string }
): 'more' | 'found' | 'gone' | 'wait' {
	void target;
	void at;
	return 'gone';
}
