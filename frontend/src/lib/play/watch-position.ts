/**
 * Where each episode was left, so opening it again resumes there.
 * Kept in the renderer's local storage under one key, for the most
 * recent episodes only. An episode left in its first seconds or its
 * last minutes has nothing to resume: the first is a start over, the
 * second is finished.
 */

/** Below this, an episode resumes from its start. */
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

/** Records `seconds` into `episode` of `showId`, or forgets the
 *  episode when that is its start or, by `duration`, its end. */
export function savePosition(
	showId: string,
	episode: number,
	seconds: number,
	duration: number,
	storage: PositionStorage | null = defaultStorage()
): void {
	const key = keyOf(showId, episode);
	const rest = load(storage).filter(([k]) => k !== key);
	if (seconds < RESUME_MIN_S || isFinishedAt(seconds, duration)) {
		store(storage, rest);
		return;
	}
	store(storage, [...rest, [key, seconds]]);
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
