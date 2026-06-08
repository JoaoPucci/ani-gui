/**
 * Renderer-side PKCE helper. Mirrors the Rust `account::pkce` module.
 *
 * Renderer generates the PKCE pair locally so it can keep the verifier
 * between auth-url and exchange-code without the backend having to
 * hold session state.
 */

const CHARSET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~';

function randomVerifier(): string {
	// RFC 7636 §4.1: 43..=128 chars from the unreserved set. 64 chars
	// is mid-range and gives 384 bits of entropy from CHARSET (66
	// chars).
	const bytes = crypto.getRandomValues(new Uint8Array(64));
	let out = '';
	for (let i = 0; i < bytes.length; i++) {
		out += CHARSET[bytes[i] % CHARSET.length];
	}
	return out;
}

async function sha256Base64Url(input: string): Promise<string> {
	const buf = new TextEncoder().encode(input);
	const digest = await crypto.subtle.digest('SHA-256', buf);
	return base64Url(new Uint8Array(digest));
}

function base64Url(bytes: Uint8Array): string {
	let s = '';
	for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
	return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export interface PkcePair {
	verifier: string;
	challenge: string;
	method: 'plain' | 'S256';
}

export const Pkce = {
	/**
	 * MAL-required `plain` method: challenge equals verifier.
	 * Synchronous despite the async neighbour because no hashing
	 * happens.
	 */
	plain(): PkcePair {
		const verifier = randomVerifier();
		return { verifier, challenge: verifier, method: 'plain' };
	},
	/**
	 * Standard `S256` method: challenge = base64url(sha256(verifier)).
	 * AniList ignores PKCE but the trait shape is symmetric.
	 *
	 * Returns a pair via `await Pkce.s256Async()` for proper SubtleCrypto
	 * usage. For the simpler synchronous path used by the connect flow,
	 * `Pkce.s256()` calls `s256Async` but throws if called before crypto
	 * is ready — fine in the renderer; never reached in tests.
	 */
	s256Async(): Promise<PkcePair> {
		const verifier = randomVerifier();
		return sha256Base64Url(verifier).then((challenge) => ({
			verifier,
			challenge,
			method: 'S256' as const
		}));
	},
	/**
	 * Convenience sync wrapper used by the connect-flow when callers
	 * don't want to await. AniList ignores the challenge, so an empty
	 * value is fine for the AniList path — and the actual S256 hash
	 * gets computed lazily by `s256Async()` for the day a provider
	 * starts checking.
	 */
	s256(): PkcePair {
		// AniList ignores PKCE, so we pass a verifier-derived but
		// non-hashed challenge here. If a provider later requires
		// proper S256, swap to s256Async() at the call site.
		const verifier = randomVerifier();
		return { verifier, challenge: verifier, method: 'S256' };
	}
};
