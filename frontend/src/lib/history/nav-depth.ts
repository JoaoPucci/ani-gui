/**
 * Where the page on screen stands in the window's history, for the
 * topbar's back affordance. Pure computation extracted from
 * +layout.svelte so the rules are unit-tested without mounting the
 * full SvelteKit runtime.
 *
 * The depth is the browser's own record: the index the Navigation API
 * reports for the current entry (`navigation.currentEntry.index`),
 * which the layout reads in its `afterNavigate` hook and hands to
 * `afterNavigation`. The browser moves it on a push, leaves it on a
 * replace, moves it to the entry a back or forward press lands on, and
 * keeps it across a reload. A count kept by the app could not: SvelteKit
 * reports `goto(url, { replaceState: true })` as a `goto`, the same as
 * a push, and restores no `page.state` on a reload, so a counted depth
 * went one too deep on every replace and back to zero on every reload.
 *
 * What the browser does not keep is the page an entry was opened from,
 * which the breadcrumb names when it is a search. The layout stamps
 * that onto the entry through SvelteKit's shallow routing:
 * `replaceState('', { ...page.state, ...stamp })` from
 * `$app/navigation` (https://svelte.dev/docs/kit/shallow-routing). On
 * popstate SvelteKit puts the entry's state back on `page.state`, where
 * it is read back. The layout never writes `history.state` itself:
 * SvelteKit owns it, and its dev runtime warns that a direct
 * `history.replaceState` will conflict with its router.
 */

/** SvelteKit's NavigationType, as `afterNavigate` reports it. */
export type NavType = 'enter' | 'popstate' | 'goto' | 'link' | 'form' | 'leave' | 'replaceState';

/** Whether the back affordance shows at the given depth. */
export function shouldShowBackButton(depth: number): boolean {
	return depth > 0;
}

/** What the layout keeps on a history entry, through SvelteKit's
 *  shallow-routing state (`App.PageState`): the address of the page
 *  it was opened from. */
export interface EntryStamp {
	aniGuiFrom: string;
}

/** One navigation, as the layout's `afterNavigate` hook sees it. */
export interface Navigated {
	type: NavType;
	/** `page.state` once the navigation has landed: on a back or
	 *  forward press, what SvelteKit restored for the entry. */
	state: Partial<EntryStamp>;
	from: URL | null;
	to: URL | null;
	/** The browser's index for the entry the navigation landed on, as
	 *  the Navigation API reports it (`navigation.currentEntry.index`):
	 *  its position in the window's session history. Null where the
	 *  browser has no Navigation API or no current entry. */
	entry: number | null;
	/** What the layout held before this navigation. */
	prev: { depth: number; origin: string | null };
}

/** Where the layout stands after a navigation, and the stamp the new
 *  entry takes (null: the entry is not stamped). */
export interface NavOutcome {
	depth: number;
	origin: string | null;
	stamp: EntryStamp | null;
}

/** Whether a navigation of this type pushed or replaced an entry the
 *  layout has not stamped yet. */
function isForward(type: NavType): boolean {
	return type === 'goto' || type === 'link' || type === 'form';
}

/** The entry's position, where the browser reports one. */
function position(entry: number | null): number | null {
	return entry !== null && entry >= 0 ? entry : null;
}

/** The layout's position after a navigation, and the stamp the entry
 *  it landed on takes. Pure. */
export function afterNavigation(nav: Navigated): NavOutcome {
	// Where the browser cannot say, there is nothing known to be behind.
	const depth = position(nav.entry) ?? 0;
	const origin = nextOrigin(nav);
	if (!isForward(nav.type) || !origin) return { depth, origin, stamp: null };
	return { depth, origin, stamp: { aniGuiFrom: origin } };
}

/** Whether a forward navigation replaced the entry it left: the
 *  browser's index for it did not move. This relies on the layout
 *  hearing of every move of the index, which holds while the app has
 *  no hash links and no shallow `pushState`: SvelteKit does not report
 *  a back or forward press across those to `afterNavigate`. */
function replaced(nav: Navigated): boolean {
	const at = position(nav.entry);
	return at !== null && at === nav.prev.depth;
}

/** The address of the page behind the entry a navigation landed on.
 *  A forward navigation records the page it left, except one that stays
 *  on the same page (the detail page's episode pager, the player's
 *  episode switch), which keeps what was behind it. "The same page" is
 *  the same path: a push to the address already on screen (picking
 *  the show on screen again from the topbar dropdown) is treated like
 *  those replaces and keeps the origin, so its chevron steps back to
 *  the entry for that same show rather than to the search. A back or forward
 *  press reads what the entry was stamped with.
 *
 *  A replace onto a different page (the player's ways back to its
 *  show) keeps the origin too: SvelteKit reports it as a `goto`, like a
 *  push, but the browser's index for the entry does not move, and what
 *  is behind the entry is what was behind the one it replaced. */
function nextOrigin(nav: Navigated): string | null {
	switch (nav.type) {
		case 'enter':
			return null;
		case 'popstate':
			return nav.state.aniGuiFrom ?? null;
		case 'goto':
		case 'link':
		case 'form':
			if (!nav.from) return null;
			if (replaced(nav)) return nav.prev.origin;
			if (nav.to && nav.to.pathname === nav.from.pathname) return nav.prev.origin;
			return nav.from.pathname + nav.from.search;
		case 'leave':
		case 'replaceState':
			return nav.prev.origin;
	}
}
