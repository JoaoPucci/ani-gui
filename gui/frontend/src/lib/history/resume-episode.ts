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
 * So the click resolves the cap itself: a cap the background probe
 * already delivered is used as-is (no extra fetch); an unknown cap
 * costs one interactive availability lookup — cache-first on the
 * backend, and the card's resume spinner is already up while it
 * runs; a failed or countless lookup falls back to Kitsu's count,
 * which is exactly the pre-probe behavior and no worse than the
 * background probe failing.
 */

import { pickNextEpisode } from '$lib/play/next-episode';

export async function resolveResumeEpisode(
	lastWatched: number | null,
	playableCount: number | null,
	kitsuCount: number | null,
	fetchCount: () => Promise<number | null>
): Promise<{ episode: number; count: number | null }> {
	if (typeof playableCount === 'number') {
		return { episode: pickNextEpisode(lastWatched, playableCount), count: playableCount };
	}
	let live: number | null;
	try {
		live = await fetchCount();
	} catch {
		live = null;
	}
	return { episode: pickNextEpisode(lastWatched, live ?? kitsuCount ?? null), count: live };
}
