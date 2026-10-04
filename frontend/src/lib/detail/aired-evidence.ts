/**
 * What the anime database's own episode rows prove has aired.
 *
 * Signatures only: the behaviour lands with the change these tests
 * describe.
 */

import type { AiringStatus } from './episode-airing';

/** The part of an episode row the evidence reads. */
export interface DatedEpisode {
	number: number | null;
	relative_number: number | null;
	airdate: string | null;
}

export function datedAired(pages: Iterable<readonly DatedEpisode[]>, nowMs: number): number {
	void pages;
	void nowMs;
	return 0;
}

export function withAiredFloor(airing: AiringStatus | null, floor: number): AiringStatus | null {
	void floor;
	return airing;
}
