import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { afterNavigation, shouldShowBackButton, type Navigated, type NavType } from './nav-depth';

describe('shouldShowBackButton', () => {
	it('hides at depth 0 (home / fresh launch)', () => {
		expect(shouldShowBackButton(0)).toBe(false);
	});

	it('shows whenever the user has navigated at least once', () => {
		expect(shouldShowBackButton(1)).toBe(true);
		expect(shouldShowBackButton(7)).toBe(true);
	});
});

const HOME = new URL('http://app.test/');
const SEARCH = new URL('http://app.test/search?q=frieren');
const SHOW = new URL('http://app.test/anime/46474');

const TYPES: NavType[] = ['enter', 'popstate', 'goto', 'link', 'form', 'leave', 'replaceState'];

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

type Step =
	| { kind: 'push'; type: 'link' | 'goto' | 'form'; path: string }
	| { kind: 'replace'; path: string }
	| { kind: 'back' }
	| { kind: 'forward' }
	| { kind: 'reload' };

const PATHS = ['/', '/search?q=a', '/anime/1', '/anime/1?page=2', '/play/1'];
const pathArb = fc.constantFrom(...PATHS);
const stepArb: fc.Arbitrary<Step> = fc.oneof(
	fc.record({
		kind: fc.constant('push' as const),
		type: fc.constantFrom('link' as const, 'goto' as const, 'form' as const),
		path: pathArb
	}),
	fc.record({ kind: fc.constant('replace' as const), path: pathArb }),
	fc.constant({ kind: 'back' as const }),
	fc.constant({ kind: 'forward' as const }),
	fc.constant({ kind: 'reload' as const })
);

/**
 * Plays a run of navigations against a model of the browser's session
 * history, the way the layout sees it: SvelteKit's type for each (a
 * `goto` for a replace as for a push; `enter` for a reload, which
 * starts a new document and restores no `page.state`), the entry's
 * index as the Navigation API reports it, and the state each entry
 * kept. After every step `check` gets the layout's depth and the
 * entry's real position.
 */
function play(start: string, steps: Step[], check: (depth: number, position: number) => void) {
	const url = (p: string) => new URL(p, 'http://app.test');
	const entries: { path: string; state: Navigated['state'] }[] = [{ path: start, state: {} }];
	let index = 0;
	const land = (
		type: NavType,
		from: string | null,
		state: Navigated['state'],
		prev: Navigated['prev']
	) => {
		const out = afterNavigation({
			type,
			state,
			from: from === null ? null : url(from),
			to: url(entries[index].path),
			entry: index,
			prev
		});
		// The layout writes its stamp onto the entry it landed on.
		if (out.stamp) entries[index].state = { ...state, ...out.stamp };
		return out;
	};

	let held = land('enter', null, {}, { depth: 0, origin: null });
	check(held.depth, index);
	for (const s of steps) {
		const from = entries[index].path;
		if (s.kind === 'push') {
			entries.splice(index + 1);
			entries.push({ path: s.path, state: {} });
			index += 1;
			held = land(s.type, from, {}, held);
		} else if (s.kind === 'replace') {
			entries[index] = { path: s.path, state: {} };
			held = land('goto', from, {}, held);
		} else if (s.kind === 'reload') {
			held = land('enter', null, {}, { depth: 0, origin: null });
		} else {
			const to = index + (s.kind === 'back' ? -1 : 1);
			if (to < 0 || to >= entries.length) continue;
			index = to;
			held = land('popstate', from, entries[index].state, held);
		}
		check(held.depth, index);
	}
}

describe('afterNavigation: the depth is the entry’s place in the session history', () => {
	it('is the index the browser reports, however the entry was reached', () => {
		for (const type of TYPES) {
			const out = afterNavigation(nav(type, { entry: 3, prev: { depth: 1, origin: null } }));
			expect(out.depth).toBe(3);
		}
	});

	it('stays where it was on a goto that replaced the entry', () => {
		// The detail page's episode pager and the player's episode switch
		// navigate with `goto(url, { replaceState: true })`, which
		// SvelteKit reports as a `goto`, the same as a push.
		const out = afterNavigation(
			nav('goto', { from: SHOW, to: SHOW, entry: 1, prev: { depth: 1, origin: '/' } })
		);
		expect(out.depth).toBe(1);
	});

	it('keeps its place across a reload, which the browser keeps', () => {
		const out = afterNavigation(
			nav('enter', { from: null, entry: 2, prev: { depth: 0, origin: null } })
		);
		expect(out.depth).toBe(2);
		expect(shouldShowBackButton(out.depth)).toBe(true);
	});

	it('is the root where the browser cannot say', () => {
		// No Navigation API, or no current entry: an index of -1 is
		// what the API reports for a document that is not fully active.
		for (const entry of [null, -1]) {
			const out = afterNavigation(nav('link', { entry, prev: { depth: 4, origin: null } }));
			expect(out.depth).toBe(0);
		}
	});

	it('equals the real position over any run of pushes, replaces, presses and reloads', () => {
		fc.assert(
			fc.property(pathArb, fc.array(stepArb, { maxLength: 40 }), (start, steps) => {
				play(start, steps, (depth, position) => {
					expect(depth).toBe(position);
					expect(shouldShowBackButton(depth)).toBe(position > 0);
				});
			})
		);
	});

	it('is never stamped on the entry', () => {
		fc.assert(
			fc.property(fc.constantFrom(...TYPES), fc.nat(50), fc.nat(50), (type, entry, depth) => {
				const out = afterNavigation(nav(type, { entry, prev: { depth, origin: '/x' } }));
				expect(out.stamp === null || !('aniGuiDepth' in out.stamp)).toBe(true);
			})
		);
	});
});

describe('afterNavigation: the page an entry was opened from', () => {
	it('records the address a forward navigation left, and stamps it on the new entry', () => {
		const out = afterNavigation(nav('link', { from: SEARCH, to: SHOW, entry: 1 }));
		expect(out.origin).toBe('/search?q=frieren');
		expect(out.stamp).toEqual({ aniGuiFrom: '/search?q=frieren' });
	});

	it('keeps the origin it had across a navigation that stays on the same page', () => {
		// The detail page's episode pager and the player's episode switch
		// change only the query; the page behind is still the one before.
		const to = new URL('http://app.test/anime/46474?page=2');
		const out = afterNavigation(
			nav('goto', { from: SHOW, to, entry: 2, prev: { depth: 2, origin: '/search?q=frieren' } })
		);
		expect(out.origin).toBe('/search?q=frieren');
		expect(out.stamp).toEqual({ aniGuiFrom: '/search?q=frieren' });
	});

	it('keeps the origin across a replace onto another page', () => {
		// The player's way back to its show replaces the player's entry:
		// what is behind the show's new entry is what was behind the
		// player's.
		const play = new URL('http://app.test/play/46474?episode=3');
		const out = afterNavigation(
			nav('goto', { from: play, to: SHOW, entry: 2, prev: { depth: 2, origin: '/search?q=a' } })
		);
		expect(out.origin).toBe('/search?q=a');
		expect(out.stamp).toEqual({ aniGuiFrom: '/search?q=a' });
	});

	it('reads the origin back from the entry on a back or forward press', () => {
		const out = afterNavigation(
			nav('popstate', {
				state: { aniGuiFrom: '/search?q=frieren' },
				entry: 2,
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

	it('stamps nothing when the navigation came from nowhere', () => {
		const out = afterNavigation(nav('goto', { from: null, entry: 1 }));
		expect(out.origin).toBeNull();
		expect(out.stamp).toBeNull();
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
						entry: depth + 1,
						prev: { depth, origin: null }
					})
				);
				const back = afterNavigation(
					nav('popstate', {
						state: pushed.stamp!,
						entry: depth + 1,
						prev: { depth: 99, origin: '/elsewhere' }
					})
				);
				expect(back.depth).toBe(pushed.depth);
				expect(back.origin).toBe(pushed.origin);
			})
		);
	});
});
