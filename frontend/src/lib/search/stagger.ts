/**
 * Entrance stagger for the search grid. A card's rise is delayed by
 * its slot, and the slot counts from zero within the batch that
 * brought it: a page appended at the bottom staggers from its own
 * first card instead of waiting behind every card already shown.
 */

import type { KitsuAnimeRef } from '$lib/api';

/** Slots for `list`: a card already in `prev` keeps its slot, a new
 *  one takes the next slot among this update's arrivals. Cards no
 *  longer listed are dropped, so one that returns rises again. */
export function staggerSlots(
	prev: Map<string, number>,
	list: KitsuAnimeRef[]
): Map<string, number> {
	const next = new Map<string, number>();
	let arrivals = 0;
	for (const hit of list) next.set(hit.id, prev.get(hit.id) ?? arrivals++);
	return next;
}
