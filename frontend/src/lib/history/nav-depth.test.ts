import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	afterNavigation,
	nextDepth,
	shouldShowBackButton,
	type Navigated,
	type NavType
} from './nav-depth';

const step = (type: NavType, prevDepth: number, stampedDepth: number | null = null) => ({
	type,
	prevDepth,
	stampedDepth
});

describe('nextDepth', () => {
	it("'enter' resets to 0 regardless of prev or stamped depth", () => {
		// Tauri's WebView used to preserve window.history.state across
		// app launches; a stamped 5 from a prior launch must not bleed
		// into the first paint.
		expect(nextDepth(step('enter', 5, 5))).toBe(0);
		expect(nextDepth(step('enter', 0, null))).toBe(0);
	});

	it("'link' / 'goto' / 'form' increment depth by one", () => {
		expect(nextDepth(step('link', 0))).toBe(1);
		expect(nextDepth(step('goto', 1))).toBe(2);
		expect(nextDepth(step('form', 2))).toBe(3);
	});

	it("'popstate' uses the new entry's stamped depth when present", () => {
		// Forward through SPA history works for free: each entry carries
		// its own depth, so popstate just reads it back.
		expect(nextDepth(step('popstate', 3, 1))).toBe(1);
		expect(nextDepth(step('popstate', 0, 2))).toBe(2);
	});

	it("'popstate' without a stamped depth decrements (back-press)", () => {
		// The original home entry of a session never gets stamped, so the
		// first back-press from depth 1 should land at 0.
		expect(nextDepth(step('popstate', 1, null))).toBe(0);
		expect(nextDepth(step('popstate', 3, null))).toBe(2);
	});

	it("'popstate' from depth 0 with no stamp clamps at 0", () => {
		// Defensive: never go negative.
		expect(nextDepth(step('popstate', 0, null))).toBe(0);
	});

	it("'leave' and 'replaceState' don't change depth", () => {
		expect(nextDepth(step('leave', 4))).toBe(4);
		expect(nextDepth(step('replaceState', 4))).toBe(4);
	});
});

describe('shouldShowBackButton', () => {
	it('hides at depth 0 (home / fresh launch)', () => {
		expect(shouldShowBackButton(0)).toBe(false);
	});

	it('shows whenever the user has navigated at least once', () => {
		expect(shouldShowBackButton(1)).toBe(true);
		expect(shouldShowBackButton(7)).toBe(true);
	});
});

describe('navigation traces — full sessions', () => {
	// End-to-end sequences modelling typical user flows. They run the
	// pure rules across a series of events and assert depth at each
	// point. These are the "if this regresses, the BackButton bug
	// returns" test cases.

	it('home → /search → back → home: button appears, then hides', () => {
		let depth = 0;
		expect(shouldShowBackButton(depth)).toBe(false);
		depth = nextDepth(step('enter', depth));
		expect(shouldShowBackButton(depth)).toBe(false);
		depth = nextDepth(step('link', depth)); // home → /search
		expect(shouldShowBackButton(depth)).toBe(true);
		depth = nextDepth(step('popstate', depth, 0)); // back → home (stamped 0)
		expect(shouldShowBackButton(depth)).toBe(false);
	});

	it('home → /search → /anime → back → back: stays visible until home', () => {
		let depth = 0;
		depth = nextDepth(step('enter', depth));
		depth = nextDepth(step('link', depth)); // depth 1, /search
		depth = nextDepth(step('link', depth)); // depth 2, /anime
		expect(shouldShowBackButton(depth)).toBe(true);
		depth = nextDepth(step('popstate', depth, 1)); // back to /search
		expect(shouldShowBackButton(depth)).toBe(true);
		depth = nextDepth(step('popstate', depth, null)); // back to home (un-stamped)
		expect(shouldShowBackButton(depth)).toBe(false);
	});
});

const HOME = new URL('http://app.test/');
const SEARCH = new URL('http://app.test/search?q=frieren');
const SHOW = new URL('http://app.test/anime/46474');

function nav(type: NavType, over: Partial<Navigated> = {}): Navigated {
	return {
		type,
		state: {},
		from: HOME,
		to: SEARCH,
		entry: null,
		prev: { depth: 0, origin: null },
		...over
	};
}

describe('afterNavigation: the depth an entry is stamped with', () => {
	it('stamps a forward navigation with its depth, one past the page it left', () => {
		for (const type of ['link', 'goto', 'form'] as const) {
			const out = afterNavigation(nav(type, { prev: { depth: 2, origin: null } }));
			expect(out.depth).toBe(3);
			expect(out.stamp).toMatchObject({ aniGuiDepth: 3 });
		}
	});

	it('reads a back or forward press from the state SvelteKit restored, and writes nothing', () => {
		const out = afterNavigation(
			nav('popstate', { state: { aniGuiDepth: 1 }, prev: { depth: 4, origin: null } })
		);
		expect(out.depth).toBe(1);
		expect(out.stamp).toBeNull();
	});

	it('steps back one on a press onto an entry nothing stamped', () => {
		expect(afterNavigation(nav('popstate', { prev: { depth: 2, origin: null } })).depth).toBe(1);
	});

	it('starts at the root on launch or reload, and writes nothing', () => {
		const out = afterNavigation(
			nav('enter', { state: { aniGuiDepth: 5 }, from: null, prev: { depth: 3, origin: null } })
		);
		expect(out).toEqual({ depth: 0, origin: null, stamp: null });
	});

	it('neither moves nor writes on a leave or a shallow replace', () => {
		for (const type of ['leave', 'replaceState'] as const) {
			const out = afterNavigation(nav(type, { prev: { depth: 2, origin: null } }));
			expect(out.depth).toBe(2);
			expect(out.stamp).toBeNull();
		}
	});

	it('stamps exactly the forward navigations, always with the depth it answers', () => {
		const types: NavType[] = ['enter', 'popstate', 'goto', 'link', 'form', 'leave', 'replaceState'];
		fc.assert(
			fc.property(
				fc.constantFrom(...types),
				fc.nat(50),
				fc.option(fc.nat(50), { nil: undefined }),
				(type, depth, stamped) => {
					const out = afterNavigation(
						nav(type, {
							state: stamped === undefined ? {} : { aniGuiDepth: stamped },
							prev: { depth, origin: null }
						})
					);
					const forward = type === 'goto' || type === 'link' || type === 'form';
					expect(out.stamp !== null).toBe(forward);
					if (out.stamp) expect(out.stamp.aniGuiDepth).toBe(out.depth);
					expect(out.depth).toBeGreaterThanOrEqual(0);
				}
			)
		);
	});
});

describe('afterNavigation: the page an entry was opened from', () => {
	it('records the address a forward navigation left, and stamps it on the new entry', () => {
		const out = afterNavigation(nav('link', { from: SEARCH, to: SHOW }));
		expect(out.origin).toBe('/search?q=frieren');
		expect(out.stamp).toEqual({ aniGuiDepth: 1, aniGuiFrom: '/search?q=frieren' });
	});

	it('keeps the origin it had across a navigation that stays on the same page', () => {
		// The detail page's episode pager and the player's episode switch
		// change only the query; the page behind is still the one before.
		const to = new URL('http://app.test/anime/46474?page=2');
		const out = afterNavigation(
			nav('goto', { from: SHOW, to, prev: { depth: 2, origin: '/search?q=frieren' } })
		);
		expect(out.origin).toBe('/search?q=frieren');
		expect(out.stamp?.aniGuiFrom).toBe('/search?q=frieren');
	});

	it('reads the origin back from the entry on a back or forward press', () => {
		const out = afterNavigation(
			nav('popstate', {
				state: { aniGuiDepth: 2, aniGuiFrom: '/search?q=frieren' },
				prev: { depth: 3, origin: '/anime/46474' }
			})
		);
		expect(out.origin).toBe('/search?q=frieren');
	});

	it('has no origin on launch, reload, or a press onto an entry nothing stamped', () => {
		expect(afterNavigation(nav('enter', { prev: { depth: 1, origin: '/x' } })).origin).toBeNull();
		expect(
			afterNavigation(nav('popstate', { prev: { depth: 1, origin: '/x' } })).origin
		).toBeNull();
	});

	it('keeps its origin on a leave or a shallow replace', () => {
		for (const type of ['leave', 'replaceState'] as const) {
			expect(afterNavigation(nav(type, { prev: { depth: 1, origin: '/x' } })).origin).toBe('/x');
		}
	});

	it('stamps no origin when the navigation came from nowhere', () => {
		const out = afterNavigation(nav('goto', { from: null }));
		expect(out.origin).toBeNull();
		expect(out.stamp).toEqual({ aniGuiDepth: 1 });
	});

	it('round-trips: a press back onto a stamped entry answers what the stamp said', () => {
		const path = fc.stringMatching(/^\/[a-z]{1,8}$/);
		fc.assert(
			fc.property(path, path, fc.nat(20), (a, b, depth) => {
				fc.pre(a !== b);
				const pushed = afterNavigation(
					nav('link', {
						from: new URL(a, 'http://app.test'),
						to: new URL(b, 'http://app.test'),
						prev: { depth, origin: null }
					})
				);
				const back = afterNavigation(
					nav('popstate', { state: pushed.stamp!, prev: { depth: 99, origin: '/elsewhere' } })
				);
				expect(back.depth).toBe(pushed.depth);
				expect(back.origin).toBe(pushed.origin);
			})
		);
	});
});
