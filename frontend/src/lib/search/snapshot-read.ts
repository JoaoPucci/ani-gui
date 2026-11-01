/**
 * Reading back a search snapshot (`snapshot.ts`). SvelteKit writes the
 * snapshot to sessionStorage, so what comes back may be one an older
 * version of the app wrote, or anything else: nothing is trusted
 * before it is checked here.
 */

import type { KitsuAnimeRef, SearchOrder } from '$lib/api';
import type { ScrollAnchor, SearchCheckpoint, SearchSnapshot } from './snapshot';
import { isObject, numOrNull, readHit, type Json } from './saved-hit';

type Check = (v: unknown) => boolean;

const isCount: Check = (v) => Number.isInteger(v) && (v as number) >= 0;
const isStrings: Check = (v) => Array.isArray(v) && v.every((x) => typeof x === 'string');
const isString: Check = (v) => typeof v === 'string';
const isNumber: Check = (v) => numOrNull(v) !== null;
const oneOf =
	(...values: unknown[]): Check =>
	(v) =>
		values.includes(v);
const countOrNull: Check = (v) => v === null || isCount(v);

/** Whether `v` is an object whose fields pass `checks`. */
const shaped = (v: unknown, checks: Record<string, Check>): v is Json =>
	isObject(v) && Object.entries(checks).every(([key, check]) => check(v[key]));

const SNAPSHOT: Record<string, Check> = {
	v: oneOf(1),
	q: isString,
	order: oneOf('relevance', 'year', 'rating'),
	subtypes: isStrings,
	capturedAt: isNumber,
	pages: isCount
};

const RESULTS: Record<string, Check> = {
	mode: oneOf('sub', 'dub'),
	hits: Array.isArray,
	seen: isStrings,
	next: countOrNull,
	emptyStreak: isCount,
	pages: isCount,
	phase: oneOf('idle', 'end', 'held'),
	fetchedAt: (v) => v === undefined || isNumber(v)
};

const ANCHOR: Record<string, Check> = { id: isString, top: isNumber };

function readResults(v: unknown): SearchCheckpoint | null {
	if (!shaped(v, RESULTS)) return null;
	const hits = (v.hits as unknown[]).map(readHit);
	if (hits.includes(null)) return null;
	const r = v as unknown as SearchCheckpoint;
	return {
		mode: r.mode,
		hits: hits as KitsuAnimeRef[],
		seen: r.seen,
		next: r.next,
		emptyStreak: r.emptyStreak,
		pages: r.pages,
		phase: r.phase,
		...(r.fetchedAt === undefined ? {} : { fetchedAt: r.fetchedAt })
	};
}

function readAnchor(v: unknown): ScrollAnchor | null {
	return shaped(v, ANCHOR) ? { id: v.id as string, top: v.top as number } : null;
}

/**
 * A snapshot SvelteKit handed back, checked: `null` when it is not one
 * this version of the page wrote. Unreadable results or an unreadable
 * anchor are dropped on their own, keeping the query and filters.
 */
export function readSnapshot(raw: unknown): SearchSnapshot | null {
	if (!shaped(raw, SNAPSHOT)) return null;
	return {
		v: 1,
		q: raw.q as string,
		order: raw.order as SearchOrder,
		subtypes: raw.subtypes as string[],
		capturedAt: raw.capturedAt as number,
		pages: raw.pages as number,
		anchor: readAnchor(raw.anchor),
		results: readResults(raw.results)
	};
}
