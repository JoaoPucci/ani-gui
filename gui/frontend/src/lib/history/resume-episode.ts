/**
 * Click-time episode resolution for a Continue Watching card.
 *
 * Cards render the moment their Kitsu match lands, which means a
 * click can arrive before the background availability probe has
 * reported the true playable cap. Kitsu's announced episode_count
 * can overshoot what's actually playable — a lagging dub is the
 * common case (watched 12, Kitsu says 24, only 12 dubbed) — and
 * advancing past the real cap forwards a phantom episode into an
 * episode-not-released error where the probed cap would have
 * replayed the last one.
 *
 * So the click resolves the cap itself: an unknown cap costs one
 * interactive availability lookup — cache-first on the backend, and
 * the card's resume spinner is already up while it runs; a failed or
 * countless lookup falls back to Kitsu's count, which is exactly the
 * pre-probe behavior and no worse than the background probe failing.
 *
 * A cap the background probe delivered is used as-is only when the
 * backend reports it as EXACT. When the scraper gate refuses the
 * detail fetch, or that fetch fails, the count comes from the search
 * hit instead — it counts half episodes as whole ones, so it runs
 * high, and by an unbounded amount when a show carries several tags.
 *
 * Provenance rather than position decides. An earlier version
 * revalidated only when the next episode was exactly the cap, which
 * silently assumed the overcount was one; with two tags the phantom
 * sits below the cap where that check never looks. So any approximate
 * cap is revalidated interactively — the lane that is neither paced
 * nor breaker-refused, and which now misses the cache, since the
 * backend refuses to serve an approximate row.
 *
 * A failed revalidation keeps the approximate number rather than
 * regressing to no cap at all. That is the residual case: it is the
 * best figure available, and it is the behaviour that shipped.
 *
 * `readCap` exists because the caller's cap is a SNAPSHOT taken when
 * the click fired, and the background probe can land while the
 * interactive lookup is still in flight. If that lookup then fails,
 * falling back to Kitsu would discard an exact cap already in hand —
 * and Kitsu's count is the very number the probe exists to correct.
 * Optional: callers with no live view of the cap simply omit it.
 */

import { pickNextEpisode } from '$lib/play/next-episode';

export async function resolveResumeEpisode(
	lastWatched: number | null,
	playableCount: number | null,
	kitsuCount: number | null,
	fetchCount: () => Promise<number | null>,
	readCap?: () => number | null,
	playableCountApproximate = false
): Promise<{ episode: number; count: number | null }> {
	if (typeof playableCount === 'number' && !playableCountApproximate) {
		return { episode: pickNextEpisode(lastWatched, playableCount), count: playableCount };
	}
	if (typeof playableCount === 'number') {
		let confirmed: number | null;
		try {
			confirmed = await fetchCount();
		} catch {
			confirmed = null;
		}
		const cap = confirmed ?? playableCount;
		return { episode: pickNextEpisode(lastWatched, cap), count: cap };
	}
	let live: number | null;
	try {
		live = await fetchCount();
	} catch {
		live = null;
	}
	// Precedence on the way down: the lookup's own answer, then a cap
	// the background probe published while it ran, then Kitsu. Only
	// the last is optimistic, so it is the last resort.
	const cap = live ?? readCap?.() ?? kitsuCount ?? null;
	return { episode: pickNextEpisode(lastWatched, cap), count: live ?? readCap?.() ?? null };
}
