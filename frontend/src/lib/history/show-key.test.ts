import { describe, expect, it } from 'vitest';
import { providerOfShowId } from './show-key';

// A history id says whose id it is: a provider's carries its label
// as a prefix, and a bare id — every row anidb.app's resolves wrote,
// and the allanime-era rows before them — names no provider the app
// walks. The renderer never parses the slug itself — only the label,
// and only to say which provider's title a cache key is for.
describe('providerOfShowId', () => {
	it('reads a labeled prefix as that provider', () => {
		expect(providerOfShowId('hianime:cowboy-bebop-1281')).toBe('hianime');
	});

	it('reads a bare id as naming no provider, the allanime-era ids included', () => {
		expect(providerOfShowId('cowboy-bebop-1281')).toBeNull();
		expect(providerOfShowId('vDTSJHSpYnrkZnAvG')).toBeNull();
		expect(providerOfShowId('')).toBeNull();
	});

	it('treats an unknown prefix as part of a bare id', () => {
		expect(providerOfShowId('weird:thing-12')).toBeNull();
	});
});
