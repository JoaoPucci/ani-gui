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
