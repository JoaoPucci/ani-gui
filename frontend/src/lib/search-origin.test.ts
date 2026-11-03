import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { searchOrigin } from '$lib/search-origin';

describe('searchOrigin', () => {
	it('answers a search the reader ran, and nothing else', () => {
		expect(searchOrigin('/search?q=frieren')).toBe('/search?q=frieren');
		for (const origin of [null, '', '/', '/search', '/search?q=%20', '/anime/1?q=a']) {
			expect(searchOrigin(origin)).toBeNull();
		}
	});

	it('answers null for an address the URL parser refuses, rather than throwing', () => {
		// `\/` reads as a scheme-relative URL with an empty host, which
		// the parser rejects. The withSearchOrigin property drew it.
		expect(searchOrigin('\\/')).toBeNull();
		expect(searchOrigin('//')).toBeNull();
	});

	it('answers null for every address shape the URL parser refuses', () => {
		// Each of these makes `new URL(origin, base)` throw: an empty or
		// malformed host behind a scheme-relative `//` (however it is
		// spelled: backslashes, leading whitespace or control
		// characters), a scheme with no host, an unclosed IPv6 host.
		for (const origin of [
			'//',
			'\\/',
			'\\\\',
			'/\\',
			' //',
			'\t//',
			'\n//',
			'\u0000//',
			'http://',
			'https://',
			'//[',
			'//a b',
			'http://[::1',
			'//%'
		]) {
			expect(() => searchOrigin(origin), JSON.stringify(origin)).not.toThrow();
			expect(searchOrigin(origin), JSON.stringify(origin)).toBeNull();
		}
	});

	it("answers null for a search that is not this app's own search page", () => {
		// The page behind is recorded as a path on this app (its
		// pathname and query, $lib/history/nav-depth). An address that
		// leaves the app, or names some other page whose path merely
		// ends in /search, is not the search the reader ran, and the
		// breadcrumb would link it as one.
		for (const origin of [
			'//search?q=a',
			'///search?q=a',
			'/\\search?q=a',
			'//evil.example/search?q=a',
			'http://app.invalid/search?q=a',
			'https://evil.example/search?q=a',
			'file:///search?q=a',
			'javascript:alert(1)//search?q=a',
			'/anime/search?q=a',
			'search?q=a',
			'\\search?q=a',
			' /search?q=a',
			'/search?q=a ',
			'/./search?q=a',
			'/x/../search?q=a',
			'/search/?q=a',
			'/search?q=a#top',
			'/search?q=a b',
			'//search?q=a b%'
		]) {
			expect(searchOrigin(origin), JSON.stringify(origin)).toBeNull();
		}
	});

	it('answers a search the reader ran however its query is encoded', () => {
		for (const origin of [
			'/search?q=a%20b',
			'/search?q=a+b',
			'/search?q=Journey%27s%20End',
			'/search?q=%E3%83%95%E3%83%AA%E3%83%BC%E3%83%AC%E3%83%B3',
			'/search?type=tv&q=a',
			'/search?q=a&page=2'
		]) {
			expect(searchOrigin(origin), origin).toBe(origin);
		}
	});

	it('never throws, and answers either nothing or the address it was given', () => {
		fc.assert(
			fc.property(
				fc.oneof(
					fc.string(),
					fc.string({ unit: 'binary' }),
					fc.webUrl({ withQueryParameters: true, withFragments: true }),
					fc
						.tuple(
							fc.constantFrom('', '/', '//', '\\', '/\\', ' ', '\t', 'http:', 'http://', 'a:'),
							fc.constantFrom('', 'search', '/search', 'search/'),
							fc.constantFrom('', '?', '?q=', '?q=a', '?q=%', '#q=a'),
							fc.string()
						)
						.map((parts) => parts.join(''))
				),
				(origin) => {
					const out = searchOrigin(origin);
					expect(out === null || out === origin).toBe(true);
				}
			),
			{ numRuns: 2000 }
		);
	});
});
