/**
 * How a tip on screen closes, by the route that closed it: whether it
 * counts as seen, and where focus goes. Rendering: the routes are the
 * screen's.
 *
 * - "Got it" closes it, seen, and focus moves to the place it
 *   explained (or back where it was, when the place takes no focus).
 * - Escape closes it, seen, and focus goes back where it was.
 * - A press outside closes it, seen, and goes no further: the press
 *   never reaches the page. A press in the first PRESS_GRACE_MS was
 *   aimed at the page before the balloon appeared, and does nothing.
 * - A press on the lit place works, and closes it, seen; focus is the
 *   press's to move. In the first PRESS_GRACE_MS the press still works
 *   but leaves the tip up.
 * - A sequence's Done (Next on its last step) closes it, seen, focus to
 *   the last place; Skip closes it, seen, focus back.
 * - An entry tip's "Show me how" closes it, seen; the press then opens
 *   the feature, where its walkthrough runs, so focus is that page's.
 * - A tip yields, unseen and still pending, when the page changes
 *   under it, its place goes, one of the app's dialogs opens over it,
 *   or the window goes fullscreen.
 */
import { PRESS_GRACE_MS } from './gate';

export type CloseRoute =
	| 'got-it'
	| 'done'
	| 'skip'
	| 'show-how'
	| 'escape'
	| 'outside'
	| 'place'
	| 'yield';

export interface Closing {
	seen: boolean;
	/** `place`: the place, else where focus was; `back`: where focus was,
	 *  else the place; `none`: the press moves it; `leave`: where the
	 *  page put it (a yield, often to a page change). */
	focus: 'place' | 'back' | 'none' | 'leave';
}

/** How the tip shown at `shownAt` closes by `route` at `now`, or null
 *  when the route does not close it. */
export function closing(route: CloseRoute, shownAt: number, now: number): Closing | null {
	switch (route) {
		case 'got-it':
		case 'done':
			return { seen: true, focus: 'place' };
		case 'skip':
			return { seen: true, focus: 'back' };
		case 'show-how':
			return { seen: true, focus: 'none' };
		case 'escape':
			return { seen: true, focus: 'back' };
		case 'outside':
			return now - shownAt < PRESS_GRACE_MS ? null : { seen: true, focus: 'back' };
		case 'place':
			return now - shownAt < PRESS_GRACE_MS ? null : { seen: true, focus: 'none' };
		case 'yield':
			return { seen: false, focus: 'leave' };
	}
}
