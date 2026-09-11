import { describe, expect, it } from 'vitest';
import { providerOfShowId } from './show-key';

// A history id says whose id it is: anidb's is the bare slug every
// existing row holds, another provider's carries its label as a
// prefix. The renderer never parses the slug itself — only the
// label, and only to say which provider's title a cache key is for.
describe('providerOfShowId', () => {
	it('reads a labeled prefix as that provider', () => {
		expect(providerOfShowId('hianime:cowboy-bebop-1281')).toBe('hianime');
	});

	it('reads a bare id as anidb, the allanime-era ids included', () => {
		expect(providerOfShowId('cowboy-bebop-1281')).toBe('anidb');
		expect(providerOfShowId('vDTSJHSpYnrkZnAvG')).toBe('anidb');
		expect(providerOfShowId('')).toBe('anidb');
	});

	it('treats an unknown prefix as part of an anidb id', () => {
		expect(providerOfShowId('weird:thing-12')).toBe('anidb');
	});
});
