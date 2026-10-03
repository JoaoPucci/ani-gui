/**
 * Where each episode was left, so opening it again resumes there.
 * Kept in the renderer's local storage under one key, for the most
 * recent episodes only. An episode left in its first seconds is kept
 * at zero — started, to start over — so Continue still goes back to
 * it; one left in its last minutes is finished and forgotten.
 */

/** Below this, an episode is kept as started, at zero. */
export const RESUME_MIN_S = 15;
/** With this little left, an episode counts as finished. */
export const FINISHED_REMAINING_S = 90;
/** How many episodes' positions are kept. */
export const MAX_POSITIONS = 200;

const KEY = 'ani-gui.watch-positions';

export type PositionStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** `[show:episode, seconds]` pairs, oldest first. */
type Positions = [string, number][];

function defaultStorage(): PositionStorage | null {
	try {
		return globalThis.localStorage ?? null;
	} catch {
		return null;
	}
}

function load(storage: PositionStorage | null): Positions {
	try {
		const parsed: unknown = JSON.parse(storage?.getItem(KEY) ?? '[]');
		return Array.isArray(parsed)
			? parsed.filter(
					(p): p is [string, number] =>
						Array.isArray(p) && typeof p[0] === 'string' && typeof p[1] === 'number'
				)
			: [];
	} catch {
		return [];
	}
}

function store(storage: PositionStorage | null, positions: Positions): void {
	try {
		storage?.setItem(KEY, JSON.stringify(positions.slice(-MAX_POSITIONS)));
	} catch {
		// A storage that refuses only loses the resume point.
	}
}

const keyOf = (showId: string, episode: number) => `${showId}:${episode}`;

/** Records `seconds` into `episode` of `showId` — zero, a started
 *  mark, when that is its start — or forgets the episode when
 *  `duration` puts it at its end. */
export function savePosition(
	showId: string,
	episode: number,
	seconds: number,
	duration: number,
	storage: PositionStorage | null = defaultStorage()
): void {
	const key = keyOf(showId, episode);
	const rest = load(storage).filter(([k]) => k !== key);
	// A point in the first seconds is a start over, never a finish —
	// on a stream shorter than its last 90 seconds as well.
	const point = seconds < RESUME_MIN_S ? 0 : seconds;
	if (point > 0 && isFinishedAt(point, duration)) {
		store(storage, rest);
		return;
	}
	store(storage, [...rest, [key, point]]);
}

/** Marks `episode` of `showId` started, at zero, unless a point is
 *  already kept for it. */
export function markStarted(
	showId: string,
	episode: number,
	storage: PositionStorage | null = defaultStorage()
): void {
	if (readPosition(showId, episode, storage) === null) {
		savePosition(showId, episode, 0, Number.NaN, storage);
	}
}

/** Whether `seconds` is in an episode's last minutes, by `duration`.
 *  A length not known yet decides nothing: the point is not finished
 *  as far as anyone can tell, and the first visit that learns the
 *  length asks again. */
export function isFinishedAt(seconds: number, duration: number): boolean {
	return Number.isFinite(duration) && duration - seconds <= FINISHED_REMAINING_S;
}

/** Where `episode` of `showId` was left, or null. */
export function readPosition(
	showId: string,
	episode: number,
	storage: PositionStorage | null = defaultStorage()
): number | null {
	const key = keyOf(showId, episode);
	return load(storage).find(([k]) => k === key)?.[1] ?? null;
}

export function clearPosition(
	showId: string,
	episode: number,
	storage: PositionStorage | null = defaultStorage()
): void {
	const key = keyOf(showId, episode);
	store(
		storage,
		load(storage).filter(([k]) => k !== key)
	);
}

/** Forgets every kept episode of `showId` — its history row is gone. */
export function clearShowPositions(
	showId: string,
	storage: PositionStorage | null = defaultStorage()
): void {
	const prefix = `${showId}:`;
	store(
		storage,
		load(storage).filter(([k]) => !k.startsWith(prefix))
	);
}

/** Forgets every kept episode — the history is cleared. */
export function clearAllPositions(storage: PositionStorage | null = defaultStorage()): void {
	store(storage, []);
}
