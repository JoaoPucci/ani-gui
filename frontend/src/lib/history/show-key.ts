/**
 * A history id says whose id it is: a provider's carries its label as
 * a prefix. A bare id — every row anidb.app's resolves wrote, and the
 * allanime-era rows before them — names no provider the app walks.
 * The renderer never parses the slug — only the label, and only to say
 * which provider's title a cache key is for.
 */
const LABELED = ['hianime'] as const;

/**
 * A provider the backend walks, by the label its show keys carry —
 * the one list on this side of the API that names providers, so a new
 * provider is one entry here (see the recipe in
 * `backend/src/provider_set.rs`).
 */
export type StreamProvider = (typeof LABELED)[number];

export function providerOfShowId(id: string): StreamProvider | null {
	for (const provider of LABELED) {
		if (id.startsWith(`${provider}:`)) return provider;
	}
	return null;
}
