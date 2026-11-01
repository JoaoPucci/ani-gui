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

/** A result as the grid shows it: what the search page renders of a
 *  card, and nothing it does not (the synopsis, the cover, the title
 *  variants), to keep the snapshot small. */
function compactHit(hit: KitsuAnimeRef): KitsuAnimeRef {
	return {
		id: hit.id,
		canonical_title: hit.canonical_title,
		slug: hit.slug,
		synopsis: null,
		start_date: hit.start_date,
		end_date: hit.end_date,
		episode_count: hit.episode_count,
		average_rating: hit.average_rating,
		subtype: hit.subtype,
		status: hit.status,
		age_rating: hit.age_rating,
		popularity_rank: hit.popularity_rank,
		poster_image: hit.poster_image,
		cover_image: null
	};
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

type Json = Record<string, unknown>;
const isObject = (v: unknown): v is Json =>
	typeof v === 'object' && v !== null && !Array.isArray(v);
const isCount = (v: unknown): v is number => Number.isInteger(v) && (v as number) >= 0;
const isStrings = (v: unknown): v is string[] =>
	Array.isArray(v) && v.every((x) => typeof x === 'string');
const strOrNull = (v: unknown) => (typeof v === 'string' ? v : null);
const numOrNull = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) ? v : null);
const ORDERS: readonly string[] = ['relevance', 'year', 'rating'];
const PHASES: readonly string[] = ['idle', 'end', 'held'];

function readPoster(v: unknown): KitsuAnimeRef['poster_image'] {
	if (!isObject(v)) return null;
	return {
		tiny: strOrNull(v.tiny),
		small: strOrNull(v.small),
		medium: strOrNull(v.medium),
		large: strOrNull(v.large),
		original: strOrNull(v.original)
	};
}

function readHit(v: unknown): KitsuAnimeRef | null {
	if (!isObject(v) || typeof v.id !== 'string' || typeof v.canonical_title !== 'string') {
		return null;
	}
	return compactHit({
		id: v.id,
		canonical_title: v.canonical_title,
		slug: strOrNull(v.slug),
		synopsis: null,
		start_date: strOrNull(v.start_date),
		end_date: strOrNull(v.end_date),
		episode_count: numOrNull(v.episode_count),
		average_rating: numOrNull(v.average_rating),
		subtype: strOrNull(v.subtype),
		status: strOrNull(v.status),
		age_rating: strOrNull(v.age_rating),
		popularity_rank: numOrNull(v.popularity_rank),
		poster_image: readPoster(v.poster_image),
		cover_image: null
	});
}

function readResults(v: unknown): SearchCheckpoint | null {
	if (!isObject(v) || (v.mode !== 'sub' && v.mode !== 'dub')) return null;
	if (!Array.isArray(v.hits) || !isStrings(v.seen) || !isCount(v.pages)) return null;
	if (!(v.next === null || isCount(v.next)) || !isCount(v.emptyStreak)) return null;
	if (typeof v.phase !== 'string' || !PHASES.includes(v.phase)) return null;
	const hits = v.hits.map(readHit);
	if (hits.some((h) => h === null)) return null;
	return {
		mode: v.mode,
		hits: hits as KitsuAnimeRef[],
		seen: v.seen,
		next: v.next,
		emptyStreak: v.emptyStreak,
		pages: v.pages,
		phase: v.phase as SearchCheckpoint['phase']
	};
}

function readAnchor(v: unknown): ScrollAnchor | null {
	if (!isObject(v) || typeof v.id !== 'string' || numOrNull(v.top) === null) return null;
	return { id: v.id, top: v.top as number };
}

/**
 * A snapshot SvelteKit handed back, checked: `null` when it is not one
 * this version of the page wrote. Unreadable results or an unreadable
 * anchor are dropped on their own, keeping the query and filters.
 */
export function readSnapshot(raw: unknown): SearchSnapshot | null {
	if (!isObject(raw) || raw.v !== 1 || typeof raw.q !== 'string') return null;
	if (typeof raw.order !== 'string' || !ORDERS.includes(raw.order)) return null;
	if (!isStrings(raw.subtypes) || numOrNull(raw.capturedAt) === null || !isCount(raw.pages)) {
		return null;
	}
	return {
		v: 1,
		q: raw.q,
		order: raw.order as SearchOrder,
		subtypes: raw.subtypes,
		capturedAt: raw.capturedAt as number,
		pages: raw.pages,
		anchor: readAnchor(raw.anchor),
		results: readResults(raw.results)
	};
}

/**
 * Whether saved results may be shown again as they are. Only on a back
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
	if (at.reload || saved.results === null || saved.results.mode !== at.mode) return false;
	const age = at.now - saved.capturedAt;
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
 * next page by itself (a page that showed nothing).
 */
export function resumeStep(
	target: ResumeTarget,
	at: { shown: readonly KitsuAnimeRef[]; pages: number; phase: string }
): 'more' | 'found' | 'gone' | 'wait' {
	if (target.id !== null && at.shown.some((h) => h.id === target.id)) return 'found';
	if (target.id === null) return 'gone';
	if (at.phase === 'loading-more') return 'wait';
	return at.phase === 'idle' && at.pages < target.pages ? 'more' : 'gone';
}
