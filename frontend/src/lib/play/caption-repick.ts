/**
 * What a captions pick on the play page does to the show's remembered
 * choice ({@link import('./caption-choice')}), and whether the page's
 * warms must run again for it.
 *
 * The backend keeps the subtitle tracks a listing past its cap has
 * room for by the choice, and the play cache keys on it
 * ({@link import('./play-cache').makeKey}): warms asked for the choice
 * before a pick answer no switch after it, so a pick that moves the
 * choice drops them and warms again. A pick the storage refused moved
 * nothing — nor does one on a session opened from a guessed match,
 * which has no show id to remember it under — and a pick of the
 * choice already remembered is written again, keeping it among the
 * most recent, but moves nothing a warm was asked for.
 */

import {
	defaultStorage,
	rememberCaption,
	storedCaption,
	type CaptionChoiceStorage
} from './caption-choice';

/** Remember `choice` for `showId`; whether the remembered choice moved,
 *  so the warms asked for the one before must run again. */
export function rememberPickMoved(
	showId: string,
	choice: string,
	storage: CaptionChoiceStorage | null = defaultStorage()
): boolean {
	const before = storedCaption(showId, storage);
	return rememberCaption(showId, choice, storage) && before !== choice;
}
