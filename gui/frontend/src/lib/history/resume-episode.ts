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
 * A cap the background probe already delivered is used as-is, with
 * one exception. That cap is not always exact: when the scraper gate
 * refuses the detail fetch the backend falls back to the search hit's
 * count and caches it, which runs one high for shows containing half
 * episodes. Standing on the boundary — where the next episode would
 * BE the cap — is the only position where that off-by-one changes the
 * answer, and it changes it into a phantom: a true finale of 12 under
 * an approximate cap of 13 would forward episode 13, which does not
 * exist. So the boundary is revalidated interactively (the lane that
 * is neither paced nor breaker-refused) and every other position
 * trusts the cap, because below it the next episode is strictly
 * inside the range either way. A failed revalidation keeps the
 * background cap rather than regressing to no cap at all.
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
	readCap?: () => number | null
): Promise<{ episode: number; count: number | null }> {
	if (typeof playableCount === 'number') {
		const next = pickNextEpisode(lastWatched, playableCount);
		// Only an ADVANCE onto the cap can be phantom. A replay of the
		// cap — the user already sat at the last episode — is proof
		// that episode exists, so it needs no confirmation.
		if (next !== playableCount || next === lastWatched) {
			return { episode: next, count: playableCount };
		}
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
