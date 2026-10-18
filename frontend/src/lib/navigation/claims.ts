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
 * (SvelteKit's `beforeNavigate`) and its own destruction. Every
 * navigation counts, the page's own included — a claim cannot tell
 * one the viewer asked for from one the page made — so an action that
 * must survive the page's own navigations does not claim.
 */

export interface NavigationClaims {
	/** Advances each time a navigation starts. */
	readonly epoch: number;
	/** Claim navigation for an action starting now; the returned
	 *  predicate says whether the claim still holds. */
	claim(): () => boolean;
	/** A navigation has started. */
	started(): void;
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
		started() {
			epoch += 1;
		},
		left() {
			gone = true;
		}
	};
}

/** The claim `claim` takes now; with no claim to take, one that always
 *  holds. For an action whose page may not report navigation. */
export function claimOrHold(claim: (() => () => boolean) | undefined): () => boolean {
	return claim ? claim() : holds;
}

const holds = () => true;
