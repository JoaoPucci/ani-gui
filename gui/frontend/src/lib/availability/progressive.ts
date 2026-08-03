/**
 * Render-then-prune availability gate for search. Split from
 * filter.ts: the probe pool plus deadline bookkeeping put the
 * combined file over the complexity ratchet's high-risk line,
 * and the two files serve different surfaces anyway — filter.ts
 * gates the passive list views, this module gates the one page
 * where the user is actively waiting on results.
 */

import {
	altTitlesFromKitsu,
	availabilityBatch,
	checkAvailability,
	yearFromKitsuRef
} from '$lib/api';
import type { KitsuAnimeRef } from '$lib/api';
import { keepCard } from './filter';

/** Inline probe pool: drains `uncached` with capped concurrency,
 *  writing each verdict into `cached`. `onVerdict` fires after
 *  every settled probe, success or failure. */
async function probeInline<T extends KitsuAnimeRef>(
	uncached: T[],
	mode: 'sub' | 'dub',
	cached: Record<string, boolean>,
	concurrency: number,
	onVerdict?: () => void
): Promise<void> {
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
					subtype: item?.subtype ?? undefined,
					kitsu_id: item.id,
					status: item.status ?? undefined
					// Interactive, unlike the rail fills: the user is
					// actively waiting on these results, and the gate's
					// paced background slots would turn a cold ~20-hit
					// search into a ~20-second wait.
				});
				cached[item.id] = r.available;
			} catch {
				// Probe failed — leave unset so we render the card
				// (lazy click path will surface the real error).
			}
			onVerdict?.();
		}
	});
	await Promise.all(workers);
}

/** Render-then-prune variant for search. Probes uncached items
 *  inline, but the caller renders through `emit` instead of a
 *  return value: if every verdict settles within `graceMs` the
 *  page gets a single flicker-free render with no known-unavailable
 *  card in it; otherwise it renders at the deadline with unknown
 *  verdicts still visible, and each late negative prunes its card.
 *  The tradeoff is deliberate — a throttled upstream once held ~20
 *  ready cards hostage for 45 seconds behind one hung probe, and a
 *  card that vanishes a beat later beats a page that never appears.
 *  Resolves once every probe has settled. */
export async function filterAvailableProgressive<T extends KitsuAnimeRef>(
	items: T[],
	mode: 'sub' | 'dub',
	emit: (visible: T[]) => void,
	graceMs = 2000,
	concurrency = 4
): Promise<void> {
	if (items.length === 0) {
		emit(items);
		return;
	}
	const ids = items.map((i) => i.id);
	let cached: Record<string, boolean> = {};
	try {
		const r = await availabilityBatch(ids, mode);
		cached = r.cached;
	} catch {
		emit(items);
		return;
	}

	// Emits are pure prunes: a card only ever leaves the list (an
	// unknown verdict keeps it, a late `true` changes nothing), so a
	// length comparison is enough to skip no-op re-renders.
	let lastLength = -1;
	const emitIfChanged = () => {
		const visible = items.filter((i) => keepCard(cached, i));
		if (visible.length === lastLength) return;
		lastLength = visible.length;
		emit(visible);
	};

	const uncached = items.filter((i) => !(i.id in cached));
	if (uncached.length === 0) {
		emitIfChanged();
		return;
	}

	let deadlinePassed = false;
	const deadline = setTimeout(() => {
		deadlinePassed = true;
		emitIfChanged();
	}, graceMs);
	await probeInline(uncached, mode, cached, concurrency, () => {
		if (deadlinePassed) emitIfChanged();
	});
	clearTimeout(deadline);
	emitIfChanged();
}
