/**
 * A search result as the search page's snapshot keeps it
 * (`snapshot.ts`): what the grid renders of a card and nothing it does
 * not, and the check that turns whatever sessionStorage handed back
 * into one.
 */

import type { KitsuAnimeRef } from '$lib/api';

/** A result as the grid shows it: what the search page renders of a
 *  card, and nothing it does not (the synopsis, the cover, the title
 *  variants), to keep the snapshot small. */
export function compactHit(hit: KitsuAnimeRef): KitsuAnimeRef {
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

export type Json = Record<string, unknown>;
export const isObject = (v: unknown): v is Json =>
	typeof v === 'object' && v !== null && !Array.isArray(v);
const strOrNull = (v: unknown) => (typeof v === 'string' ? v : null);
export const numOrNull = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) ? v : null);

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

/** A saved result read back: `null` without an id and a title; any
 *  other field of the wrong shape reads as absent. */
export function readHit(v: unknown): KitsuAnimeRef | null {
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
