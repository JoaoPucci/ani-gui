/**
 * How a notice arrives: a toast, held for later, or only the bell.
 *
 * - The update never toasts: its badge pops into the topbar instead.
 * - While a video plays, only a failure of the app's own (a download
 *   that failed) toasts; everything else, a published notice of any
 *   priority included, waits in the list and the bell's count.
 * - Nothing ever appears over fullscreen, native or the player's own:
 *   a toast that would show is held, and shows on leaving fullscreen
 *   if its row is still there and unread. The bell's count and the
 *   list change at once either way.
 */
import { toneOf, type Notice } from './model';

/** What is going on when a notice arrives. */
export interface Moment {
	/** A video is playing (in the window or picture-in-picture). */
	playing: boolean;
	/** Something is fullscreen. */
	fullscreen: boolean;
}

export const IDLE: Moment = { playing: false, fullscreen: false };

export type Arrival = 'toast' | 'hold' | 'quiet';

export function arrival(n: Notice, fresh: boolean, at: Moment): Arrival {
	if (!fresh || n.kind === 'update') return 'quiet';
	if (at.playing && (toneOf(n) !== 'failure' || n.kind === 'remote')) return 'quiet';
	return at.fullscreen ? 'hold' : 'toast';
}

/** The held toasts to show now, and what stays held. Out of
 *  fullscreen, every held row still present, unread, and allowed to
 *  toast at this moment shows once; the rest are let go. */
export function release(
	held: readonly string[],
	list: readonly Notice[],
	at: Moment
): { show: Notice[]; held: string[] } {
	if (at.fullscreen) return { show: [], held: [...held] };
	const show: Notice[] = [];
	for (const key of new Set(held)) {
		const n = list.find((r) => r.key === key);
		if (n && !n.read && arrival(n, true, at) === 'toast') show.push(n);
	}
	return { show, held: [] };
}
