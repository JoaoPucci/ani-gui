/**
 * Tests for the renderer-side PKCE helper.
 *
 * The plain method goes onto the wire for MAL; the S256 path is the
 * symmetric counterpart for AniList (which ignores PKCE) and a future
 * provider. Both must produce verifiers in the RFC 7636 §4.1 charset.
 */

import { describe, expect, it } from 'vitest';
import { Pkce } from './pkce';

const CHARSET_RE = /^[A-Za-z0-9\-._~]+$/;

describe('Pkce.plain', () => {
	it('returns a pair where challenge equals verifier (RFC 7636 §4.4)', () => {
		const p = Pkce.plain();
		expect(p.challenge).toBe(p.verifier);
		expect(p.method).toBe('plain');
	});

	it('produces verifiers in the RFC 7636 §4.1 charset', () => {
		for (let i = 0; i < 8; i++) {
			const p = Pkce.plain();
			expect(p.verifier).toMatch(CHARSET_RE);
		}
	});

	it('produces verifiers in the 43..=128 length range', () => {
		const p = Pkce.plain();
		expect(p.verifier.length).toBeGreaterThanOrEqual(43);
		expect(p.verifier.length).toBeLessThanOrEqual(128);
	});

	it('returns a fresh verifier on each call', () => {
		const a = Pkce.plain();
		const b = Pkce.plain();
		expect(a.verifier).not.toBe(b.verifier);
	});
});

describe('Pkce.s256', () => {
	it('reports the S256 method on the wire', () => {
		const p = Pkce.s256();
		expect(p.method).toBe('S256');
	});

	it('produces a verifier in the charset', () => {
		const p = Pkce.s256();
		expect(p.verifier).toMatch(CHARSET_RE);
	});

	it('returns a fresh verifier on each call', () => {
		const a = Pkce.s256();
		const b = Pkce.s256();
		expect(a.verifier).not.toBe(b.verifier);
	});
});

describe('Pkce.s256Async', () => {
	it('challenge = base64url-no-pad(SHA-256(verifier))', async () => {
		const p = await Pkce.s256Async();
		expect(p.method).toBe('S256');
		expect(p.challenge).not.toContain('=');
		expect(p.challenge).not.toContain('+');
		expect(p.challenge).not.toContain('/');
		// Match the same crypto used by the helper to verify the
		// challenge is in fact the SHA-256 of the verifier — not just
		// any string.
		const buf = new TextEncoder().encode(p.verifier);
		const digest = await crypto.subtle.digest('SHA-256', buf);
		const expected = Array.from(new Uint8Array(digest))
			.map((b) => String.fromCharCode(b))
			.join('');
		const expectedB64 = btoa(expected).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
		expect(p.challenge).toBe(expectedB64);
	});
});
