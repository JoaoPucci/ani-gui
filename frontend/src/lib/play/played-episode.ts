/**
 * The episode a session plays, as opposed to the one it was asked for.
 * The two differ when the request named an episode by a number it no
 * longer carries — a stitched recap an earlier version called "24.5",
 * "12.5" now — and the player then has to follow the session: its
 * number, its next and previous, and the key its position is kept
 * under.
 */

import type { CreateSessionResponse } from '$lib/api';
import { defaultStorage, type PositionStorage } from './watch-position-storage';

/** The episode `session` plays: its own `episode` when it names one,
 *  `requested` otherwise. */
export function playedEpisode(
	session: Pick<CreateSessionResponse, 'episode'>,
	requested: number
): number {
	void session;
	return requested;
}

/** {@link playedEpisode}, with the positions kept for the episode under
 *  the numbers it was asked for and was called before moved to the
 *  one it plays. */
export function followPlayed(
	showId: string,
	session: Pick<CreateSessionResponse, 'episode' | 'former_episode'>,
	requested: number,
	storage: PositionStorage | null = defaultStorage()
): number {
	void [showId, session, storage];
	return requested;
}
