/**
 * Whether an action a page started may still navigate.
 *
 * A click that awaits work and then calls `goto` can answer after the
 * viewer has moved on. The page then navigates them somewhere they
 * did not ask to go, from a page they have left or over a navigation
 * they started since. A page claims navigation when the action starts
 * and asks the claim before it navigates.
 *
 * The page is the adapter: it reports each navigation that starts
 * (SvelteKit's `beforeNavigate`) and its own destruction.
 *
 * A claim lapses on a navigation that leaves the page: another
 * pathname — another route, or another show, whose id is in the path
 * — another origin, or out of the app (`to` is null, as it is for an
 * unload). A navigation that changes only the query keeps the page on
 * screen as it is, and the action is still the viewer's: the detail
 * page's episode pager, the player's own episode switch, or a link or
 * goto to the URL already on screen. A navigation reported without
 * both ends counts as leaving.
 */

/** The part of SvelteKit's `beforeNavigate` navigation a claim reads. */
export interface NavigationLike {
	from?: { url: URL } | null;
	to?: { url: URL } | null;
}

export interface NavigationClaims {
	/** Advances each time a navigation that leaves the page starts. */
	readonly epoch: number;
	/** Claim navigation for an action starting now; the returned
	 *  predicate says whether the claim still holds. */
	claim(): () => boolean;
	/** A navigation has started; `nav` is what SvelteKit's
	 *  `beforeNavigate` reports about it. */
	started(nav?: NavigationLike): void;
	/** The page has gone. */
	left(): void;
}

export function createNavigationClaims(): NavigationClaims {
	let epoch = 0;
	let gone = false;
	return {
		get epoch() {
			return epoch;
		},
		claim() {
			const at = epoch;
			return () => !gone && epoch === at;
		},
		started(nav) {
			if (nav && staysOnPage(nav)) return;
			epoch += 1;
		},
		left() {
			gone = true;
		}
	};
}

/** Whether `nav` keeps the page on screen: the same origin and path,
 *  only the query changing. */
function staysOnPage(nav: NavigationLike): boolean {
	const from = nav.from?.url;
	const to = nav.to?.url;
	if (!from || !to) return false;
	return from.origin === to.origin && from.pathname === to.pathname;
}

/** The claim `claim` takes now; with no claim to take, one that always
 *  holds. For an action whose page may not report navigation. */
export function claimOrHold(claim: (() => () => boolean) | undefined): () => boolean {
	return claim ? claim() : holds;
}

const holds = () => true;
