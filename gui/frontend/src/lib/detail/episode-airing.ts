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
	if (n <= airing.aired) return { unaired: false };
	return {
		unaired: true,
		airsAt: n === airing.next_episode ? airing.next_airing_at : null
	};
}

/**
 * Filter a prefetch target list down to aired episodes. The detail
 * page's background warm must not spend scraper slots resolving
 * greyed-out future episodes (Codex P2 #3565590966); unknown airing
 * data passes everything through, mirroring {@link epAirState}.
 */
export function airedTargets(targets: number[], airing: AiringStatus | null): number[] {
	return targets.filter((n) => !epAirState(n, airing).unaired);
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
