/**
 * How the show page's watch-order strip arrives without pushing the
 * page: the room it holds while the franchise walk runs, and how that
 * room gives way.
 *
 * The walk can take a second or two on a first visit, and the strip
 * (or the franchise link on its own) used to land above the synopsis
 * after the page was drawn, pushing everything under it down. The
 * walk's first read, the show's own relation listing, already says
 * what is coming (`kitsuFranchiseFirstHop`), so:
 *
 * - The page waits for that first hop together with the show's
 *   details, but never more than `FIRST_HOP_CAP_MS` past the details,
 *   and no longer once the walk (and the order setting) have answered,
 *   when nothing would be held; a first hop that answers later is not
 *   used, and the strip arrives as it always did.
 * - While the walk runs, the page holds the room of what the first hop
 *   foretold (`heldShape`): a quiet outline of the strip, or of the
 *   link. Nothing is held when it foretold nothing.
 * - When the walk answers with something to draw, the outline goes at
 *   once and the strip fades in where it stood, from the outline's
 *   height to its own (`arriveOf`). When it answers with nothing (or
 *   fails), the outline fades, then folds away (`leaveOf`).
 * - Where the outline is already above the viewport, the reader is
 *   reading something under it: it goes at once, and the page scrolls
 *   to where it was, moved by what the change gave back or took
 *   (`scrollBack`; the scroll itself is `./scroll-hold`), so what they
 *   read stays where it is.
 *
 * Reduced motion makes every step instant (`$lib/motion`).
 */
import type { FirstHop } from '$lib/api';

export type { FirstHop };

/** What the page holds room for. */
export type Shape = 'strip' | 'link';

/** How long past the show's details the page waits for the first hop. */
export const FIRST_HOP_CAP_MS = 300;
/** The strip fading in where the outline stood. */
export const ARRIVE_MS = 220;
/** The outline fading, when nothing comes. */
export const LEAVE_FADE_MS = 100;
/** The outline folding away, after it faded. */
export const LEAVE_COLLAPSE_MS = 260;

/**
 * The details, with the first hop if it answered by the time they did
 * or within `FIRST_HOP_CAP_MS` after; null if not, if it failed, or if
 * `walked` settled first: on the page, the walk and the order setting,
 * or, for a show remembered this session, the order setting alone.
 * Rejects only as `details` does.
 */
export async function withFirstHop<T>(
	details: Promise<T>,
	hop: Promise<FirstHop>,
	wait: (ms: number) => Promise<void>,
	walked: Promise<unknown> = new Promise<never>(() => {})
): Promise<{ details: T; hop: FirstHop | null }> {
	// Read now, so a hop that answers before the details is not lost,
	// and a hop that fails is no hop rather than a failure.
	const answered = hop.then(
		(h): FirstHop | null => h,
		() => null
	);
	const shown = await details;
	const capped = wait(FIRST_HOP_CAP_MS).then(() => null);
	// Once `walked` settles nothing is held, so nothing is waited for.
	const moot = walked.then(
		() => null,
		() => null
	);
	return { details: shown, hop: await Promise.race([answered, capped, moot]) };
}

/** The room held while the walk has not answered, or null. */
export function heldShape(hop: FirstHop | null, answered: boolean): Shape | null {
	if (answered || hop === null || hop === 'none') return null;
	return hop;
}

/** Where the held room was, in the viewport, when the walk answered. */
export interface Gone {
	readonly top: number;
	readonly height: number;
}

/** The held room going: a fade, then a fold. */
export interface Leave {
	readonly fade: number;
	readonly collapse: number;
}

/** How the held room goes: at once where something takes its place or it is above the viewport. */
export function leaveOf(drawn: boolean, gone: Gone | null): Leave {
	if (drawn || above(gone)) return { fade: 0, collapse: 0 };
	return { fade: LEAVE_FADE_MS, collapse: LEAVE_COLLAPSE_MS };
}

/** What was drawn arriving: a fade, from the held room's height. */
export interface Arrive {
	readonly fade: number;
	readonly from: number | null;
}

/** How the strip or link arrives: where nothing was held, or the room was above the viewport, at once. */
export function arriveOf(gone: Gone | null): Arrive {
	if (gone === null || above(gone)) return { fade: 0, from: null };
	return { fade: ARRIVE_MS, from: gone.height };
}

/** The arriving block's style at `t` (0 to 1), growing or shrinking from `from` to `to` pixels. */
export function arriveCss(from: number | null, to: number, t: number): string {
	const opacity = `opacity: ${t};`;
	if (from === null || from === to) return opacity;
	return `${opacity} block-size: ${from + (to - from) * t}px; overflow: hidden;`;
}

/** How far to scroll once the held room has gone, so what is read under it stays put. */
export function scrollBack(gone: Gone | null, drawnHeight: number): number {
	return gone !== null && above(gone) ? drawnHeight - gone.height : 0;
}

/** Whether the held room starts above the viewport. */
function above(gone: Gone | null): boolean {
	return gone !== null && gone.top < 0;
}
