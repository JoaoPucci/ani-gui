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
 * It reports provenance alongside the count, because "prefer the
 * published cap" is only safe while that cap is exact: against an
 * approximate snapshot, an equally approximate publication is not an
 * improvement, and only a confirmed one displaces it.
 * Optional: callers with no live view of the cap simply omit it.
 */

import { pickNextEpisode } from '$lib/play/next-episode';

export async function resolveResumeEpisode(
	lastWatched: number | null,
	playableCount: number | null,
	kitsuCount: number | null,
	fetchCount: () => Promise<{ count: number | null; approximate: boolean }>,
	readCap?: () => { count: number | null; approximate: boolean } | null,
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
		const confirmed = await answered(fetchCount);
		if (confirmed) return at(lastWatched, confirmed);
		// The revalidation gave nothing. A cap the background probe
		// published while it ran still beats the snapshot, but only
		// when it is EXACT — swapping one unconfirmed number for
		// another buys nothing.
		const published = readCap?.() ?? null;
		if (published?.count != null && !published.approximate) return at(lastWatched, published);
		// Otherwise keep the snapshot, still unconfirmed: the next
		// click revalidates again rather than inheriting a false
		// confirmation.
		return at(lastWatched, { count: playableCount, approximate: true });
	}
	const live = await answered(fetchCount);
	// Precedence on the way down: the lookup's own answer, then a cap
	// the background probe published while it ran, then Kitsu. Only
	// the last is optimistic, so it is the last resort — and unlike
	// the other two it is not an answer, so it sets the episode
	// without being reported as a cap.
	const answer = live ?? readCap?.() ?? null;
	if (answer?.count != null) return at(lastWatched, answer);
	return {
		episode: pickNextEpisode(lastWatched, kitsuCount ?? null),
		count: null,
		// `approximate` describes a CAP, and there is none.
		approximate: false
	};
}

/**
 * A lookup result that actually carries a count. A throw and a
 * `{count: null}` answer are the same thing to every caller here: no
 * cap was established, so nothing about the existing one was
 * confirmed either.
 */
async function answered(
	fetchCount: () => Promise<{ count: number | null; approximate: boolean }>
): Promise<{ count: number; approximate: boolean } | null> {
	try {
		const r = await fetchCount();
		return r.count == null ? null : { count: r.count, approximate: r.approximate };
	} catch {
		return null;
	}
}

function at(lastWatched: number | null, cap: { count: number | null; approximate: boolean }) {
	return {
		episode: pickNextEpisode(lastWatched, cap.count),
		count: cap.count,
		approximate: cap.approximate
	};
}
