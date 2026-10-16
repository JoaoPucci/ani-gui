/**
 * The name an episode tile falls back to when Kitsu has none for it.
 * Kitsu names whole episodes only, so a fractional tile — a recap the
 * provider tags 7.5, or an extra it lists after a show's last episode
 * (see docs/title-resolution.md, "Episode numbers") — always falls
 * back.
 */

/** The fallback name for tile `num`, in the page's own wording: its
 *  `episode` name for a whole episode, its `extra` name for a
 *  fractional one. */
export function fallbackTileTitle(
	num: number | null,
	episode: (num: string) => string,
	extra: (num: string) => string
): string {
	void extra;
	return episode(String(num ?? ''));
}
