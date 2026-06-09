// Picks the PKCE method appropriate for the provider's auth-url endpoint.
// AniList accepts both methods and ignores the challenge; MAL rejects S256
// (backend returns UnsupportedPkce → 400) so the renderer must generate a
// `plain` pair for it. Implementation lands in the green commit.

import type { Provider } from './types';
import type { PkcePair } from './pkce';

export function pkceForProvider(provider: Provider): PkcePair {
	throw new Error(`pkceForProvider: not implemented for ${provider}`);
}
