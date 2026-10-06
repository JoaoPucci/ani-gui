/**
 * The click on an episode the schedule calls unaired, start to finish —
 * shared by the detail grid and the player strip, which differ only in
 * the adapters they hand in.
 *
 * The sequence: refuse while the page is busy; raise the block with
 * this check's caption; ask the controller ($lib/detail/unaired-recheck)
 * to refresh the schedule and the episode's Kitsu page, applied only if
 * the page is still the one that asked; then act on the answer — play
 * an episode that turned out aired, hand one past the provider's
 * catalogued count to the provider re-ask, or say it still has not
 * aired, or that the check failed.
 *
 * A check that lands after the user left lets go of the block only
 * while it is still the one this check raised, told apart by its
 * caption: the route component is reused across shows, and the block
 * may by then belong to an action the new show started.
 */

import {
	planUnairedClick,
	unairedRecheck,
	type UnairedClickPlan,
	type UnairedRecheckDeps,
	type UnairedRecheckOutcome,
	type UnairedRecheckSubject
} from './unaired-recheck';

/** The controller's surface the click needs — the app-wide one unless
 *  a test hands in its own. */
export interface UnairedRecheckController {
	check: <T>(
		subject: UnairedRecheckSubject,
		deps: UnairedRecheckDeps<T>
	) => Promise<UnairedRecheckOutcome>;
}

export interface UnairedClickDeps<T> {
	/** The show on screen, or null before the route knows it. */
	show: string | null;
	/** The clicked episode, in Kitsu numbering. */
	episode: number;
	/** Episodes per Kitsu page — which page the refresh fetches. */
	kitsuPageSize: number;
	/** The block's caption while this check runs. */
	caption: string;
	/** Whether the page is blocked on something already. */
	isBusy: () => boolean;
	/** Raise the block with `caption`. */
	hold: (caption: string) => void;
	/** The caption the block shows now. */
	heldCaption: () => string | null;
	/** Let go of the block. */
	release: () => void;
	/** Refetch the schedule and the page past their caches; no writes. */
	fetch: (show: string, page: number) => Promise<T>;
	/** Store what `fetch` returned. Runs only for the page that asked. */
	apply: (fresh: T, page: number) => void;
	/** Whether the episode reads as aired from what the page holds. */
	isAired: () => boolean;
	/** Whether the episode sits past the provider's catalogued count. */
	beyondPlayable: () => boolean;
	currentContext: () => string;
	play: () => void;
	recheckProvider: () => void;
	notifyStillUnaired: () => void;
	notifyFailed: () => void;
	now?: () => number;
	recheck?: UnairedRecheckController;
}

/** Run one click. Resolves to what was done — `ignored` when the page
 *  was busy or had no show. */
export async function runUnairedClick<T>(
	deps: UnairedClickDeps<T>
): Promise<UnairedClickPlan | 'ignored'> {
	const show = deps.show;
	if (deps.isBusy() || !show) return 'ignored';
	const page = Math.ceil(deps.episode / deps.kitsuPageSize);
	deps.hold(deps.caption);
	const outcome = await (deps.recheck ?? unairedRecheck).check<T>(
		{ show, page },
		{
			refresh: () => deps.fetch(show, page),
			apply: (fresh) => deps.apply(fresh, page),
			isAired: deps.isAired,
			currentContext: deps.currentContext,
			now: deps.now ?? (() => Date.now())
		}
	);
	const plan = planUnairedClick(outcome, deps.beyondPlayable());
	if (plan === 'none' && deps.heldCaption() !== deps.caption) return plan;
	deps.release();
	if (plan === 'play') deps.play();
	else if (plan === 'recheck-provider') deps.recheckProvider();
	else if (plan === 'still-unaired') deps.notifyStillUnaired();
	else if (plan === 'failed') deps.notifyFailed();
	return plan;
}
