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
	checkAvailability,
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
	const filtered = items.filter((i) => cached[i.id] !== false || !unavailableMayHide(i.status));

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
	return items.filter((i) => cached[i.id] !== false || !unavailableMayHide(i.status));
}

/** Strict variant: probes uncached items inline (parallel, capped
 *  concurrency) before returning. Use on surfaces where the user
 *  is actively waiting for results — e.g. search — and would
 *  rather wait a beat than see unavailable cards rendered. Home
 *  uses the fire-and-forget {@link filterAvailable} so cards
 *  don't disappear mid-session. */
export async function filterAvailableStrict<T extends KitsuAnimeRef>(
	items: T[],
	mode: 'sub' | 'dub',
	concurrency = 4
): Promise<T[]> {
	if (items.length === 0) return items;
	const ids = items.map((i) => i.id);
	let cached: Record<string, boolean> = {};
	try {
		const r = await availabilityBatch(ids, mode);
		cached = r.cached;
	} catch {
		return items;
	}

	const uncached = items.filter((i) => !(i.id in cached));
	if (uncached.length > 0) {
		const queue = uncached.slice();
		const workers = Array.from({ length: Math.min(concurrency, queue.length) }, async () => {
			while (queue.length > 0) {
				const item = queue.shift();
				if (!item) break;
				try {
					const r = await checkAvailability({
						title: item.canonical_title,
						mode,
						alt_titles: altTitlesFromKitsu(item),
						episode_count: item.episode_count ?? undefined,
						year: yearFromKitsuRef(item) ?? undefined,
						kitsu_id: item.id,
						status: item.status ?? undefined
					});
					cached[item.id] = r.available;
				} catch {
					// Probe failed — leave unset so we render the card
					// (lazy click path will surface the real error).
				}
			}
		});
		await Promise.all(workers);
	}

	return items.filter((i) => cached[i.id] !== false || !unavailableMayHide(i.status));
}
