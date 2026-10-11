/**
 * The storage under watch-position.ts: the one local-storage key the
 * kept positions live under, and reading and writing them. Split from
 * it so each file stays inside the CRAP gate's per-file bar.
 */

/** How many episodes' positions are kept. */
export const MAX_POSITIONS = 200;

const KEY = 'ani-gui.watch-positions';

export type PositionStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** `[show:episode, seconds]` entries, oldest first. One a session a
 *  Continue card opened wrote carries the card's history row — its
 *  provider show id — third: removing the card forgets it, whatever
 *  show the card names by then. The latest write decides it. */
export type Position = [string, number] | [string, number, string];
export type Positions = Position[];

export function defaultStorage(): PositionStorage | null {
	try {
		return globalThis.localStorage ?? null;
	} catch {
		return null;
	}
}

export function load(storage: PositionStorage | null): Positions {
	try {
		const parsed: unknown = JSON.parse(storage?.getItem(KEY) ?? '[]');
		return Array.isArray(parsed)
			? parsed.filter(
					(p): p is Position =>
						Array.isArray(p) &&
						typeof p[0] === 'string' &&
						typeof p[1] === 'number' &&
						(p[2] === undefined || typeof p[2] === 'string')
				)
			: [];
	} catch {
		return [];
	}
}

/** Whether the storage took the write. */
export function store(storage: PositionStorage | null, positions: Positions): boolean {
	try {
		storage?.setItem(KEY, JSON.stringify(positions.slice(-MAX_POSITIONS)));
		return true;
	} catch {
		// A storage that refuses only loses the resume point.
		return false;
	}
}

export const keyOf = (showId: string, episode: number) => `${showId}:${episode}`;
