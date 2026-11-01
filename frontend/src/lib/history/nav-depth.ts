/**
 * SPA back-stack depth tracker. Pure computation extracted from
 * +layout.svelte so we can unit-test the rules without mounting the
 * full SvelteKit runtime.
 *
 * The layout calls `afterNavigation(...)` from inside `afterNavigate`,
 * then stamps the result onto the new history entry through SvelteKit's
 * shallow routing: `replaceState('', { ...page.state, ...stamp })` from
 * `$app/navigation` (https://svelte.dev/docs/kit/shallow-routing). On
 * popstate SvelteKit puts the entry's state back on `page.state`, so
 * the stamped depth is read back from there, and forward+back through
 * the SPA history works without us tracking direction. The layout
 * never writes `history.state` itself: SvelteKit owns it, and its dev
 * runtime warns that a direct `history.replaceState` will conflict
 * with its router.
 *
 * Why this is its own module:
 *   - The original tracker lived inline in the layout effect, which
 *     made it functionally untested. The fix that hid the BackButton
 *     correctly went through several false starts before this one
 *     stuck — all of which could have been caught by a 10-line unit
 *     test if the rules had been a function.
 *   - Extracting the rules also makes the layout effect a thin
 *     adapter: it hands the SvelteKit event and `page.state` to
 *     afterNavigation and writes the stamp it returns through
 *     SvelteKit's replaceState. No conditional logic in the
 *     component file.
 */

/** Subset of SvelteKit's NavigationType we react to. Other variants
 *  (e.g. 'leave', 'replaceState') don't change the depth. */
export type NavType = 'enter' | 'popstate' | 'goto' | 'link' | 'form' | 'leave' | 'replaceState';

export interface NavStep {
	/** SvelteKit afterNavigate's `type` field. */
	type: NavType;
	/** Depth read from `page.state.aniGuiDepth` (the value stamped on
	 *  the entry we're moving TO). Null when not stamped —
	 *  pre-stamp entries from earlier sessions, or types that don't
	 *  carry our state. */
	stampedDepth: number | null;
	/** Depth tracked in component state right before this navigation. */
	prevDepth: number;
}

/** Compute the new depth for a single navigation event. Pure. */
export function nextDepth(step: NavStep): number {
	switch (step.type) {
		case 'enter':
			// Fresh app load (or hard reload). Reset to root regardless of
			// any leftover history.state from a prior session — Tauri's
			// WebView used to preserve state across launches, and without
			// this reset the BackButton showed up immediately on open.
			return 0;
		case 'popstate':
			// Read the new entry's stamped depth. If absent (the very
			// first home entry of a session never gets stamped), fall
			// back to decrementing — popstate in this app is always a
			// back-press because there's no forward affordance in the UI.
			return step.stampedDepth ?? Math.max(0, step.prevDepth - 1);
		case 'goto':
		case 'link':
		case 'form':
			// Forward navigation. Layout caller is responsible for
			// stamping the resulting depth onto the new entry's state via
			// SvelteKit's replaceState.
			return step.prevDepth + 1;
		case 'leave':
		case 'replaceState':
			// Neither of these changes our position in the back stack.
			return step.prevDepth;
	}
}

/** Whether the BackButton should render at the given depth. */
export function shouldShowBackButton(depth: number): boolean {
	return depth > 0;
}

/** What the layout keeps on a history entry, through SvelteKit's
 *  shallow-routing state (`App.PageState`): the entry's depth in the
 *  back stack, and the address of the page it was opened from. */
export interface EntryStamp {
	aniGuiDepth: number;
	aniGuiFrom?: string;
}

/** One navigation, as the layout's `afterNavigate` hook sees it. */
export interface Navigated {
	type: NavType;
	/** `page.state` once the navigation has landed: on a back or
	 *  forward press, what SvelteKit restored for the entry. */
	state: Partial<EntryStamp>;
	from: URL | null;
	to: URL | null;
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

/** The layout's position after a navigation, and the stamp the entry
 *  it landed on takes. Pure. */
export function afterNavigation(nav: Navigated): NavOutcome {
	const stamped = nav.state.aniGuiDepth;
	const depth = nextDepth({
		type: nav.type,
		stampedDepth: typeof stamped === 'number' ? stamped : null,
		prevDepth: nav.prev.depth
	});
	const origin = nextOrigin(nav);
	if (!isForward(nav.type)) return { depth, origin, stamp: null };
	return {
		depth,
		origin,
		stamp: origin ? { aniGuiDepth: depth, aniGuiFrom: origin } : { aniGuiDepth: depth }
	};
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
 *  A replace navigation onto a different page cannot be told from a
 *  push here (SvelteKit reports both as `goto`), so it records the page
 *  it replaced. The app makes one (the player's own fallback to the
 *  show page), and the address it records is not a search, which is
 *  the only origin the breadcrumb acts on. */
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
			if (nav.to && nav.to.pathname === nav.from.pathname) return nav.prev.origin;
			return nav.from.pathname + nav.from.search;
		case 'leave':
		case 'replaceState':
			return nav.prev.origin;
	}
}
