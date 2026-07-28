/**
 * List-view availability gate. Reads the backend's cache via the
 * batch endpoint, drops cards we KNOW are unavailable, and fires a
 * background warm so the next visit's cache is fuller. The caller
 * never sees cards disappear mid-session — filtering is a snapshot
 * taken before render; warming runs concurrent and silent.
 */

import {
	altTitlesFromKitsu,
	availabilityBatch,
	availabilityWarm,
	yearFromKitsuRef
} from '$lib/api';
import type { KitsuAnimeRef } from '$lib/api';

/** Whether an `available: false` verdict may hide this card. Only a
 *  FINISHED show missing from allmanga is confidently gone; upcoming
 *  seasons exist on Kitsu before allmanga catalogs them, and airing
 *  shows can lag the catalog the same way. Those stay visible so the
 *  user can open the page and plan them — the detail page's play
 *  surfaces gate themselves on availability + airing separately.
 *  Unknown status keeps the card: hide only on confident evidence. */
function unavailableMayHide(status: string | null | undefined): boolean {
	return status === 'finished';
}

/** The shared drop-by-cache predicate all the variants filter with
 *  (including the render-then-prune one in progressive.ts). */
export function keepCard(cached: Record<string, boolean>, item: KitsuAnimeRef): boolean {
	return cached[item.id] !== false || !unavailableMayHide(item.status);
}

/** Filter `items` against the availability cache, then warm uncached
 *  entries in the background. Returns the filtered list immediately;
 *  the warm Promise is intentionally swallowed (fire-and-forget). */
export async function filterAvailable<T extends KitsuAnimeRef>(
	items: T[],
	mode: 'sub' | 'dub'
): Promise<T[]> {
	if (items.length === 0) return items;
	const ids = items.map((i) => i.id);
	let cached: Record<string, boolean> = {};
	try {
		const r = await availabilityBatch(ids, mode);
		cached = r.cached;
	} catch {
		// Cache fetch failed — render everything; lazy click path
		// still surfaces real errors.
		return items;
	}
	const filtered = items.filter((i) => keepCard(cached, i));

	// Fire-and-forget warm for any item not in the cache. Skipping
	// items whose availability is already known keeps the queue
	// short.
	const toWarm = items
		.filter((i) => !(i.id in cached))
		.map((i) => ({
			title: i.canonical_title,
			mode,
			alt_titles: altTitlesFromKitsu(i),
			episode_count: i.episode_count ?? undefined,
			year: yearFromKitsuRef(i) ?? undefined,
			kitsu_id: i.id,
			status: i.status ?? undefined
		}));
	if (toWarm.length > 0) {
		void availabilityWarm(toWarm).catch(() => {});
	}

	return filtered;
}

/** Cache-only variant: same drop-by-cache shape as {@link filterAvailable}
 *  but skips the fire-and-forget warm entirely. Use on surfaces that
 *  fire often — the topbar live-search, where every settled keystroke
 *  would otherwise enqueue redundant upstream probes for overlapping
 *  hits. The cache fills via other surfaces (home rows, detail page);
 *  the dropdown is just a quick-jump aid and doesn't need to actively
 *  prime the cache. */
export async function filterAvailableCacheOnly<T extends KitsuAnimeRef>(
	items: T[],
	mode: 'sub' | 'dub'
): Promise<T[]> {
	if (items.length === 0) return items;
	const ids = items.map((i) => i.id);
	let cached: Record<string, boolean> = {};
	try {
		const r = await availabilityBatch(ids, mode);
		cached = r.cached;
	} catch {
		// Cache fetch failed — render everything; lazy click path
		// still surfaces real errors.
		return items;
	}
	return items.filter((i) => keepCard(cached, i));
}
