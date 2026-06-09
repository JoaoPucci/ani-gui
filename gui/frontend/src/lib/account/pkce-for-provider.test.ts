import { describe, expect, it } from 'vitest';
import { pkceForProvider } from './pkce-for-provider';

describe.skip('pkceForProvider [red — green commit unskips]', () => {
	it('picks S256 for anilist (provider ignores the challenge but accepts both)', () => {
		const pair = pkceForProvider('anilist');
		expect(pair.method).toBe('S256');
		expect(pair.verifier.length).toBeGreaterThanOrEqual(43);
		expect(pair.challenge.length).toBeGreaterThan(0);
	});

	it('picks plain for mal (MAL spec forbids S256, backend returns UnsupportedPkce otherwise)', () => {
		const pair = pkceForProvider('mal');
		expect(pair.method).toBe('plain');
		// plain method requires challenge == verifier (RFC 7636 §4.4).
		expect(pair.challenge).toBe(pair.verifier);
		expect(pair.verifier.length).toBeGreaterThanOrEqual(43);
	});
});
