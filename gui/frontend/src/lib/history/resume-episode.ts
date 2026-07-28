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
	fetchCount: () => Promise<{ count: number | null; approximate: boolean }>,
	readCap?: () => number | null,
	playableCountApproximate = false
): Promise<{ episode: number; count: number | null; approximate: boolean }> {
	if (typeof playableCount === 'number' && !playableCountApproximate) {
		return {
			episode: pickNextEpisode(lastWatched, playableCount),
			count: playableCount,
			approximate: false
		};
	}
	if (typeof playableCount === 'number') {
		let confirmed: { count: number | null; approximate: boolean } | null;
		try {
			confirmed = await fetchCount();
		} catch {
			confirmed = null;
		}
		// A revalidation that fails, or that comes back approximate
		// itself, leaves the cap approximate — the next click has to
		// revalidate again rather than inherit a false confirmation.
		const cap = confirmed?.count ?? playableCount;
		return {
			episode: pickNextEpisode(lastWatched, cap),
			count: cap,
			approximate: confirmed?.approximate ?? true
		};
	}
	let live: { count: number | null; approximate: boolean } | null;
	try {
		live = await fetchCount();
	} catch {
		live = null;
	}
	// Precedence on the way down: the lookup's own answer, then a cap
	// the background probe published while it ran, then Kitsu. Only
	// the last is optimistic, so it is the last resort.
	const cap = live?.count ?? readCap?.() ?? kitsuCount ?? null;
	const resolved = live?.count ?? readCap?.() ?? null;
	return {
		episode: pickNextEpisode(lastWatched, cap),
		count: resolved,
		// `approximate` describes a CAP, so with no cap it is false.
		// Otherwise: a lookup that answered carries its own provenance,
		// while falling back to the loader's value leaves it as
		// unconfirmed as it already was.
		approximate: resolved === null ? false : live?.count != null ? live.approximate : true
	};
}
