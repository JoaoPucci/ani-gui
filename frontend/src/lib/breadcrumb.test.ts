import { describe, expect, it } from 'vitest';
import {
	defaultTrailFor,
	goesBack,
	searchOrigin,
	labelCells,
	withSearchOrigin,
	yieldingSegment
} from './breadcrumb';
import fc from 'fast-check';
import { m } from '$lib/paraglide/messages';

describe('defaultTrailFor', () => {
	it('returns a single Home segment on the home route', () => {
		expect(defaultTrailFor('/')).toEqual([{ label: m.breadcrumb_home() }]);
	});

	it('returns a single Home segment when the route id is null', () => {
		// Hard-loaded paths before SvelteKit has resolved the route id —
		// a one-segment trail keeps the breadcrumb visible without
		// claiming a child page until we know which one it is.
		expect(defaultTrailFor(null)).toEqual([{ label: m.breadcrumb_home() }]);
	});

	it('puts each top-level route under Home with a clickable parent', () => {
		// Each known route maps to a labelled child segment. The Home
		// parent must be href-set so clicking it walks back up the
		// tree. Labels resolve through Paraglide so a locale switch
		// re-renders them.
		const cases: Array<[string, string]> = [
			['/search', m.breadcrumb_search()],
			['/search?q=foo', m.breadcrumb_search()],
			['/settings', m.breadcrumb_settings()],
			['/diagnostics', m.breadcrumb_diagnostics()],
			['/about', m.breadcrumb_about()],
			['/anime/[id]', m.breadcrumb_anime()],
			['/play/[id]', m.breadcrumb_watching()]
		];
		for (const [route, label] of cases) {
			const trail = defaultTrailFor(route);
			expect(trail[0]).toEqual({ label: m.breadcrumb_home(), href: '/' });
			expect(trail[1]).toEqual({ label });
		}
	});

	it('falls back to a bare Home parent for unknown routes', () => {
		// Defensive — a route added after the breadcrumb still renders
		// something rather than collapsing to "[empty]".
		expect(defaultTrailFor('/some-future-route')).toEqual([
			{ label: m.breadcrumb_home(), href: '/' }
		]);
	});
});

describe('withSearchOrigin', () => {
	const home = { label: m.breadcrumb_home(), href: '/' };
	const show = { label: 'Frieren' };

	it('names the search a page was opened from, between Home and the page', () => {
		expect(withSearchOrigin([home, show], '/search?q=frieren', '/anime/[id]')).toEqual([
			home,
			{ label: m.breadcrumb_search(), href: '/search?q=frieren' },
			show
		]);
	});

	it('leaves the trail alone when the page was not opened from a search', () => {
		for (const origin of [null, '/', '/anime/1', '/search', '/search?q=', '/searching?q=a']) {
			expect(withSearchOrigin([home, show], origin, '/anime/[id]')).toEqual([home, show]);
		}
	});

	it('leaves the search page its own trail', () => {
		const trail = defaultTrailFor('/search');
		expect(withSearchOrigin(trail, '/search?q=a', '/search')).toEqual(trail);
	});

	it('leaves an empty trail empty', () => {
		expect(withSearchOrigin([], '/search?q=a', '/anime/[id]')).toEqual([]);
	});

	it('does not name the search twice', () => {
		const trail = [home, { label: m.breadcrumb_search(), href: '/search?q=a' }, show];
		expect(withSearchOrigin(trail, '/search?q=a', '/anime/[id]')).toEqual(trail);
	});

	it('only ever adds the one crumb, and keeps the page last', () => {
		fc.assert(
			fc.property(
				fc.array(
					fc.record({ label: fc.string(), href: fc.option(fc.string(), { nil: undefined }) }),
					{
						minLength: 1,
						maxLength: 5
					}
				),
				fc.option(fc.string(), { nil: null }),
				(trail, origin) => {
					const out = withSearchOrigin(trail, origin, '/anime/[id]');
					expect(out.length - trail.length).toBeGreaterThanOrEqual(0);
					expect(out.length - trail.length).toBeLessThanOrEqual(1);
					expect(out.at(-1)).toEqual(trail.at(-1));
				}
			)
		);
	});

	it('replays the run that drew `//` as the origin (fast-check seed 877037045)', () => {
		// This seed's 25th case draws the origin `//or%`, which the URL
		// parser refuses (a malformed host behind `//`); fast-check
		// shrank it to `//`. The property above failed on it before
		// searchOrigin caught the refusal.
		fc.assert(
			fc.property(
				fc.array(
					fc.record({ label: fc.string(), href: fc.option(fc.string(), { nil: undefined }) }),
					{ minLength: 1, maxLength: 5 }
				),
				fc.option(fc.string(), { nil: null }),
				(trail, origin) => {
					const out = withSearchOrigin(trail, origin, '/anime/[id]');
					expect(out.length - trail.length).toBeGreaterThanOrEqual(0);
					expect(out.length - trail.length).toBeLessThanOrEqual(1);
					expect(out.at(-1)).toEqual(trail.at(-1));
				}
			),
			{ seed: 877037045 }
		);
		expect(withSearchOrigin([home, show], '//', '/anime/[id]')).toEqual([home, show]);
	});
});

describe('goesBack', () => {
	it('goes back for the address of the page behind, and only for it', () => {
		expect(goesBack('/search?q=a', '/search?q=a')).toBe(true);
		expect(goesBack('/', '/search?q=a')).toBe(false);
		expect(goesBack('/search?q=a', null)).toBe(false);
	});
});

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

describe('yieldingSegment', () => {
	const home = { label: 'Home', href: '/' };
	const title = 'JoJo no Kimyou na Bouken Part 6: Stone Ocean';

	it("gives way with the show's title on the show page and in the player", () => {
		expect(
			yieldingSegment([home, { label: 'Search', href: '/search?q=jojo' }, { label: title }])
		).toBe(2);
		expect(yieldingSegment([home, { label: title, href: '/anime/1' }, { label: 'EP 3' }])).toBe(1);
	});

	it('takes the last of equally long labels, and nothing from an empty trail', () => {
		expect(
			yieldingSegment([{ label: 'Home', href: '/' }, { label: 'Anime' }, { label: 'About' }])
		).toBe(2);
		expect(yieldingSegment([])).toBe(-1);
	});

	it('counts a full-width title by the cells it draws', () => {
		expect(labelCells('Home')).toBe(4);
		expect(labelCells('進撃の巨人')).toBe(10);
		expect(labelCells('ｶﾞﾝﾀﾞﾑ')).toBe(6); // half-width kana and their marks stay narrow
		expect(labelCells('Ｆｕｌｌ')).toBe(8);
		expect(yieldingSegment([home, { label: 'Search', href: '/s' }, { label: '進撃の巨人' }])).toBe(
			2
		);
	});

	it('names a segment no other outlasts, and none after it as long', () => {
		fc.assert(
			fc.property(
				fc.array(fc.record({ label: fc.string({ unit: 'grapheme' }) }), {
					minLength: 1,
					maxLength: 6
				}),
				(trail) => {
					const i = yieldingSegment(trail);
					expect(i).toBeGreaterThanOrEqual(0);
					expect(i).toBeLessThan(trail.length);
					const len = labelCells;
					trail.forEach((seg, j) => {
						expect(len(seg.label)).toBeLessThanOrEqual(len(trail[i].label));
						if (j > i) expect(len(seg.label)).toBeLessThan(len(trail[i].label));
					});
				}
			)
		);
	});
});
