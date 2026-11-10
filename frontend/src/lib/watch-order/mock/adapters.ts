/**
 * Small adapters for the mockup components: the PosterCard shape and a
 * rail's scroll offset; the franchise's display name is re-exported
 * from `./franchise-name`.
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

/**
 * The scroll offset that brings an item into a horizontal scroller at
 * `align` of the free width (0 the start, 0.5 the middle, 1 the end),
 * clamped to the scrollable range. `itemStart` is measured from the
 * start of the scroller's content. The rail puts the entry being
 * viewed late in the window, so what came before it stays in sight.
 */
export function scrollLeftFor(
	box: { clientWidth: number; scrollWidth: number },
	itemStart: number,
	itemWidth: number,
	align: number
): number {
	const target = itemStart - (box.clientWidth - itemWidth) * align;
	return Math.max(0, Math.min(target, box.scrollWidth - box.clientWidth));
}

export { franchiseName } from './franchise-name';
