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
