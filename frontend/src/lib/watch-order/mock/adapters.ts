/**
 * Small adapters for the mockup components: the PosterCard shape; the
 * franchise's display name is re-exported from `./franchise-name`.
 * Part of the mockups' views; `./view` gathers them.
 */
import type { KitsuAnimeRef } from '$lib/api';
import type { MockEntry } from './types';

/** A fixture entry in the shape PosterCard takes. */
export function asRef(entry: MockEntry): KitsuAnimeRef {
	return {
		id: entry.id,
		canonical_title: entry.title,
		slug: null,
		synopsis: null,
		start_date: entry.start,
		end_date: null,
		episode_count: entry.episodes,
		average_rating: null,
		subtype: entry.subtype,
		status: entry.status,
		age_rating: null,
		popularity_rank: null,
		poster_image: entry.poster
			? { tiny: null, small: null, medium: entry.poster, large: null, original: null }
			: null,
		cover_image: entry.cover
			? { tiny: null, small: null, large: entry.cover, original: null }
			: null
	};
}

export { franchiseName } from './franchise-name';
