/**
 * What the anime database's own episode rows prove has aired.
 *
 * The unaired-episode gate reads the schedule source's aired count,
 * and that count can describe a different entry than the one the page
 * shows. A show the anime database keeps as one entry can be split in
 * two by the schedule source — Steel Ball Run's March premiere is its
 * own one-episode, finished entry there, and that is the entry the
 * show maps to — so the schedule says one episode is out while the
 * database dates three. Followed alone, the stalest source wins and
 * real episodes render as unaired.
 *
 * A dated episode row is evidence in one direction only: it proves an
 * episode is out, never that one is pending. So it can raise the aired
 * count and nothing else — an unknown schedule stays unknown (unknown
 * gates nothing), and a schedule already at or past the floor is
 * returned untouched.
 */

import type { AiringStatus } from './episode-airing';

/** The part of an episode row the evidence reads. */
export interface DatedEpisode {
	number: number | null;
	relative_number: number | null;
	airdate: string | null;
}

const DAY_MS = 86_400_000;
const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

/** The episode a row is, numbered the way the tiles number it, or null
 *  when it is not a whole, positive episode. */
function wholeEpisode(row: DatedEpisode): number | null {
	const n = row.number ?? row.relative_number;
	return n !== null && Number.isInteger(n) && n >= 1 ? n : null;
}

/** True once the row's air date is over everywhere. A date carries no
 *  hour; on the day itself the schedule, which does, is the better
 *  witness. The end of the UTC day covers a Japanese broadcast day
 *  whole, late-night slots included. */
function dateIsOver(airdate: string | null, nowMs: number): boolean {
	if (airdate === null || !ISO_DATE.test(airdate)) return false;
	const start = Date.parse(`${airdate}T00:00:00Z`);
	return Number.isFinite(start) && start + DAY_MS <= nowMs;
}

/**
 * The highest whole episode whose air date is over, across every page
 * given. A later dated episode proves the undated ones before it.
 */
export function datedAired(pages: Iterable<readonly DatedEpisode[]>, nowMs: number): number {
	let best = 0;
	for (const page of pages) {
		for (const row of page) {
			const n = wholeEpisode(row);
			if (n !== null && n > best && dateIsOver(row.airdate, nowMs)) best = n;
		}
	}
	return best;
}

/**
 * Raise a schedule's aired count to `floor`. Anything the schedule
 * announced at or below the floor is dropped from its future: no tile
 * may be both out and announced for a date.
 */
export function withAiredFloor(airing: AiringStatus | null, floor: number): AiringStatus | null {
	if (airing === null || airing.aired === null || floor <= airing.aired) return airing;
	const nextPassed = airing.next_episode !== null && airing.next_episode <= floor;
	const raised: AiringStatus = {
		...airing,
		aired: floor,
		next_episode: nextPassed ? null : airing.next_episode,
		next_airing_at: nextPassed ? null : airing.next_airing_at
	};
	if (airing.upcoming !== undefined) {
		raised.upcoming = airing.upcoming.filter((u) => u.episode > floor);
	}
	return raised;
}
