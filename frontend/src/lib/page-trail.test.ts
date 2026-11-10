import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { get, writable } from 'svelte/store';
import { defaultTrailFor, type BreadcrumbSegment } from './breadcrumb';
import { createTrailKeeper, trailAfterReset } from './page-trail';
import { m } from '$lib/paraglide/messages';

describe('the trail a page names for itself', () => {
	const title: BreadcrumbSegment[] = [
		{ label: m.breadcrumb_home(), href: '/' },
		{ label: 'Frieren' }
	];
	const keeper = () => {
		const store = writable<BreadcrumbSegment[]>([]);
		return { store, trail: createTrailKeeper(store) };
	};

	it('survives the reset of the navigation that landed on its page', () => {
		// SvelteKit reports a navigation once the page it lands on has
		// rendered, so a page can name itself first — a navigation to the
		// address on screen keeps the page and its name; the reset that
		// follows must not take the name away.
		const { store, trail } = keeper();
		trail.name('/anime/1', title);
		trail.reset('/anime/1', '/anime/[id]');
		expect(get(store)).toEqual(title);
	});

	it('replaces the default when it lands after the reset', () => {
		const { store, trail } = keeper();
		trail.reset('/anime/1', '/anime/[id]');
		expect(get(store)).toEqual(defaultTrailFor('/anime/[id]'));
		trail.name('/anime/1', title);
		expect(get(store)).toEqual(title);
	});

	it('survives a navigation that changes only the query, as the episode pager does', () => {
		const { store, trail } = keeper();
		trail.reset('/anime/1', '/anime/[id]');
		trail.name('/anime/1', title);
		trail.reset('/anime/1', '/anime/[id]');
		expect(get(store)).toEqual(title);
	});

	it('is dropped by the navigation to another page, and stays dropped on the way back', () => {
		const { store, trail } = keeper();
		trail.name('/anime/1', title);
		trail.reset('/search', '/search');
		expect(get(store)).toEqual(defaultTrailFor('/search'));
		trail.reset('/anime/1', '/anime/[id]');
		expect(get(store)).toEqual(defaultTrailFor('/anime/[id]'));
	});

	const pathArb = fc.constantFrom('/', '/search', '/anime/1', '/anime/2', '/play/1', '/settings');
	const routeOf = (path: string) =>
		path === '/' ? '/' : path.replace(/^\/(anime|play)\/\d+$/, '/$1/[id]');
	const trailArb = fc.array(
		fc.record({ label: fc.string(), href: fc.option(fc.string(), { nil: undefined }) }),
		{ minLength: 1, maxLength: 4 }
	);

	it('reads the page’s own trail at the end, whichever of reset and name comes first', () => {
		fc.assert(
			fc.property(
				fc.array(fc.tuple(pathArb, trailArb), { maxLength: 6 }),
				pathArb,
				trailArb,
				fc.boolean(),
				(before, path, own, nameFirst) => {
					const { store, trail } = keeper();
					// Whatever happened earlier in the session.
					for (const [p, t] of before) {
						trail.reset(p, routeOf(p));
						trail.name(p, t);
					}
					if (nameFirst) {
						trail.name(path, own);
						trail.reset(path, routeOf(path));
					} else {
						trail.reset(path, routeOf(path));
						trail.name(path, own);
					}
					expect(get(store)).toEqual(own);
				}
			)
		);
	});

	it('never carries one page’s trail onto another page', () => {
		fc.assert(
			fc.property(pathArb, pathArb, trailArb, (named, landed, own) => {
				fc.pre(named !== landed);
				expect(trailAfterReset({ path: named, trail: own }, landed, routeOf(landed))).toEqual(
					defaultTrailFor(routeOf(landed))
				);
			})
		);
	});
});
