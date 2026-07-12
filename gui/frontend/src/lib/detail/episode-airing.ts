/**
 * Unaired-episode gating for the detail page's episode tiles.
 *
 * The backend's `/api/kitsu/airing/:kitsu_id` reports how many
 * episodes have actually aired (AniList's schedule), so tiles past
 * that count render greyed + non-clickable instead of inviting a
 * doomed source resolution (Yani Neko: 12 announced, 2 out). Pure
 * helpers so the policy is unit-testable without the page.
 */

/** Mirror of the backend's `meta::anilist::AiringStatus` wire shape. */
export interface AiringStatus {
	/** Episodes aired so far; null = unknown (never gate on unknown). */
	aired: number | null;
	/** Number of the next episode to air, when scheduled. */
	next_episode: number | null;
	/** Epoch seconds of the next airing, when scheduled. */
	next_airing_at: number | null;
	/** Published future schedule (episode number → air time). Weekly
	 *  shows publish a few weeks ahead; optional for tolerance to
	 *  payloads written before the field existed. */
	upcoming?: { episode: number; airing_at: number }[];
}

/** Per-tile verdict: aired tiles stay interactive; unaired ones grey
 *  out, and the very next episode carries its air date for the label. */
export type EpAirState = { unaired: false } | { unaired: true; airsAt: number | null };

/**
 * Decide episode `n`'s tile state. Unknown airing data (fetch failed,
 * show unmapped, no schedule) must leave every tile interactive —
 * hiding real episodes on a guess is worse than a doomed click.
 */
export function epAirState(n: number, airing: AiringStatus | null): EpAirState {
	if (airing == null || airing.aired == null) return { unaired: false };
	// Floor: allmanga's decimal extras (a 2.5 recap airs between the
	// regular eps 2 and 3) aren't counted by AniList's schedule, so a
	// strict compare would grey a released special until the NEXT
	// regular episode airs (Codex P2 #3565610386).
	if (Math.floor(n) <= airing.aired) return { unaired: false };
	// Prefer the published schedule (every episode gets its own date);
	// fall back to nextAiringEpisode for shows without a schedule list.
	const scheduled = airing.upcoming?.find((u) => u.episode === n)?.airing_at ?? null;
	return {
		unaired: true,
		airsAt: scheduled ?? (n === airing.next_episode ? airing.next_airing_at : null)
	};
}

/**
 * True while the airing question is still being *asked* — a
 * non-finished show whose airing fetch hasn't settled. Distinct from
 * resolved-unknown (fetch failed, show unmapped), which must never
 * gate: availability can return from cache before `airingGet`
 * finishes, and treating that in-flight beat as unknown leaves tiles
 * and the primary CTA interactive long enough for a quick click to
 * start resolving an unaired episode (Codex P2 #3565710325).
 * Finished shows skip the fetch; a missing Kitsu status means no
 * fetch ever starts — neither may report pending, or the gate would
 * never lift.
 */
export function airingPending(resolved: boolean, status: string | null | undefined): boolean {
	if (resolved) return false;
	return status != null && status !== 'finished';
}

/**
 * Short localized air-date label for the next-episode tile
 * (e.g. "Jul 17"). Epoch seconds in, display string out.
 */
export function formatAirDate(epochSeconds: number, locale: string): string {
	return new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric' }).format(
		new Date(epochSeconds * 1000)
	);
}
