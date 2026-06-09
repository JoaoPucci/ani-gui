// Picks the PKCE method appropriate for the provider's auth-url endpoint.
// AniList accepts both methods and ignores the challenge; MAL rejects S256
// (backend returns UnsupportedPkce → 400) so the renderer must generate a
// `plain` pair for it.

import { Pkce, type PkcePair } from './pkce';
import type { Provider } from './types';

export function pkceForProvider(provider: Provider): PkcePair {
	if (provider === 'mal') return Pkce.plain();
	return Pkce.s256();
}
