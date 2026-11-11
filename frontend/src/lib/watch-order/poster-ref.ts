/**
 * A franchise entry in the shape PosterCard takes, for the franchise
 * page's rails of other versions, films, side stories and recaps.
 */
import type { KitsuAnimeRef } from '$lib/api';
import type { WatchEntry } from './types';

export function asRef(entry: WatchEntry): KitsuAnimeRef {
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
