/**
 * A history id says whose id it is: anidb's is the bare slug every
 * existing row holds, another provider's carries its label as a
 * prefix. The renderer never parses the slug — only the label, and
 * only to say which provider's title a cache key is for.
 */
export type StreamProvider = 'anidb' | 'hianime';

const LABELED: ReadonlyArray<StreamProvider> = ['hianime'];

export function providerOfShowId(id: string): StreamProvider {
	for (const provider of LABELED) {
		if (id.startsWith(`${provider}:`)) return provider;
	}
	return 'anidb';
}
