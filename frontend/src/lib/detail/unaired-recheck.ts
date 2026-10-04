/**
 * Re-asking the schedule when a user clicks an episode the page calls
 * unaired.
 *
 * The schedule row and the episode page are both cached — hours for
 * the one, a day for the other — and the schedule can describe a
 * different entry than the page shows, so a tile can read unaired
 * well after its episode came out. The click is the user asking
 * whether that is still true, so it refreshes both past their caches
 * and re-reads the episode from what the page then holds.
 *
 * ONE REFRESH PER SHOW PER INTERVAL. Ten clicks on a greyed tile are
 * one question; the limit lasts the app session (the controller is a
 * module singleton) so leaving and re-opening the page does not reset
 * it. Inside the interval the page's own state is already as fresh as
 * the last refresh made it, so the click is answered from it. Clicks
 * that land while a refresh is out join it. A refresh that failed
 * established nothing and does not count against the limit; nor does
 * one that landed after the user left, since no page applied it.
 *
 * NOTHING LANDS IN A PAGE THAT DID NOT ASK. The route component is
 * reused across shows, so the refresh only fetches; its result is
 * applied after the context check, never from inside the refresh.
 */

export const UNAIRED_RECHECK_INTERVAL_MS = 5 * 60_000;

export type UnairedRecheckOutcome = 'aired' | 'unaired' | 'failed' | 'superseded';

export interface UnairedRecheckDeps<T> {
	/** Refetch the schedule and the episode's page past their caches.
	 *  Returns what it fetched and writes nothing: the page may no
	 *  longer be the one that asked by the time it lands. */
	refresh: () => Promise<T>;
	/** Store what the refresh fetched where the page derives its tiles
	 *  from. Called only while the context is still the one that asked. */
	apply: (fresh: T) => void;
	/** Whether the episode reads as aired from what the page holds. */
	isAired: () => boolean;
	/** What the answer is about — compared when it lands, so an answer
	 *  for a page the user has left is neither applied nor reported. */
	currentContext: () => string;
	now: () => number;
}

/** A refresh's result: what it fetched and when it started, or null
 *  when it failed. Untyped here because clicks from different pages of
 *  the same show share one; each page's `apply` knows its own shape. */
type Fetched = { data: unknown; startedAt: number } | null;

export function createUnairedRecheck(intervalMs: number = UNAIRED_RECHECK_INTERVAL_MS): {
	check: <T>(showId: string, deps: UnairedRecheckDeps<T>) => Promise<UnairedRecheckOutcome>;
} {
	const refreshedAt = new Map<string, number>();
	const inFlight = new Map<string, Promise<Fetched>>();

	function refreshOnce<T>(showId: string, deps: UnairedRecheckDeps<T>): Promise<Fetched> {
		const pending = inFlight.get(showId);
		if (pending) return pending;
		const startedAt = deps.now();
		const started = deps
			.refresh()
			.then(
				(data): Fetched => ({ data, startedAt }),
				(): Fetched => null
			)
			.finally(() => inFlight.delete(showId));
		inFlight.set(showId, started);
		return started;
	}

	return {
		check: async <T>(showId: string, deps: UnairedRecheckDeps<T>) => {
			const asked = deps.currentContext();
			const last = refreshedAt.get(showId);
			const recent = last !== undefined && deps.now() - last < intervalMs;
			if (recent && !inFlight.has(showId)) return deps.isAired() ? 'aired' : 'unaired';
			const fetched = await refreshOnce(showId, deps);
			if (deps.currentContext() !== asked) return 'superseded';
			if (fetched === null) return 'failed';
			deps.apply(fetched.data as T);
			// The limit starts once a page holds the answer: until then a
			// repeat click answered from page state would answer from the
			// very cache it is questioning.
			refreshedAt.set(showId, fetched.startedAt);
			return deps.isAired() ? 'aired' : 'unaired';
		}
	};
}

/** The app-wide controller both routes share. */
export const unairedRecheck = createUnairedRecheck();

export type UnairedClickPlan = 'play' | 'recheck-provider' | 'still-unaired' | 'failed' | 'none';

/**
 * What a route does with the answer. An episode that turned out aired
 * is handled as any aired tile is: played, or — past the provider's
 * catalogued count — handed to the provider re-ask.
 */
export function planUnairedClick(
	outcome: UnairedRecheckOutcome,
	beyondPlayable: boolean
): UnairedClickPlan {
	switch (outcome) {
		case 'aired':
			return beyondPlayable ? 'recheck-provider' : 'play';
		case 'unaired':
			return 'still-unaired';
		case 'failed':
			return 'failed';
		case 'superseded':
			return 'none';
	}
}
