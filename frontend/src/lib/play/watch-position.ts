/**
 * Where each episode was left, so opening it again resumes there.
 * Kept in the renderer's local storage under one key, for the most
 * recent episodes only. An episode left in its first seconds is kept
 * at zero — started, to start over — so Continue still goes back to
 * it; one left in its last minutes is finished and forgotten.
 */

import { recoveryResume } from './resume-after-recovery';
import { nextWrite, picks, writes } from './write-order';

/** Below this, an episode is kept as started, at zero. */
export const RESUME_MIN_S = 15;
/** With this little left, an episode counts as finished. */
export const FINISHED_REMAINING_S = 90;
/** How many episodes' positions are kept. */
export const MAX_POSITIONS = 200;

const KEY = 'ani-gui.watch-positions';

export type PositionStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** `[show:episode, seconds]` entries, oldest first. One a session a
 *  Continue card opened wrote carries the card's history row — its
 *  provider show id — third: removing the card forgets it, whatever
 *  show the card names by then. The latest write decides it. */
type Position = [string, number] | [string, number, string];
type Positions = Position[];

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
function store(storage: PositionStorage | null, positions: Positions): boolean {
	try {
		storage?.setItem(KEY, JSON.stringify(positions.slice(-MAX_POSITIONS)));
		return true;
	} catch {
		// A storage that refuses only loses the resume point.
		return false;
	}
}

const keyOf = (showId: string, episode: number) => `${showId}:${episode}`;

/** Records `seconds` into `episode` of `showId` — zero, a started
 *  mark, when that is its start — or forgets the episode when
 *  `duration` puts it at its end. `row` is the history row whose
 *  Continue card opened the session, when one did. */
export function savePosition(
	showId: string,
	episode: number,
	seconds: number,
	duration: number,
	storage: PositionStorage | null = defaultStorage(),
	row: string | null = null
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
	if (store(storage, [...rest, row ? [key, point, row] : [key, point]])) writes.note(storage, key);
}

/** Marks `episode` of `showId` started, at zero, for `row` as
 *  savePosition does, unless a point is already kept for it. A pick
 *  of such a point is noted: a removal's cleanup begun before it,
 *  which forgets the point, leaves a started mark in its place. */
export function markStarted(
	showId: string,
	episode: number,
	storage: PositionStorage | null = defaultStorage(),
	row: string | null = null
): void {
	if (readPosition(showId, episode, storage) === null) {
		savePosition(showId, episode, 0, Number.NaN, storage, row);
	} else {
		picks.note(storage, keyOf(showId, episode));
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

/** The moment a removal's rows are gone. Its cleanup, handed this as
 *  `since`, forgets only what was written before it. */
export function snapshotPositions(): number {
	return nextWrite();
}

/** Forgets every kept episode of `showId` — its history row is gone —
 *  and a recovery's pending point for it: with `since`, only those
 *  written before it. */
export function clearShowPositions(
	showId: string,
	storage: PositionStorage | null = defaultStorage(),
	since?: number
): void {
	recoveryResume.forgetShow(showId, since);
	const prefix = `${showId}:`;
	const kept: Positions = [];
	for (const p of load(storage)) {
		const left = p[0].startsWith(prefix) ? afterRemoval(p, storage, since) : p;
		if (left) kept.push(left);
	}
	store(storage, kept);
}

/** What a removal's cleanup leaves of `p`, kept before `since`: all of
 *  it when written since, a started mark when its episode was picked
 *  since, and otherwise nothing. */
function afterRemoval(
	p: Position,
	storage: PositionStorage | null,
	since?: number
): Position | null {
	if (writes.since(storage, p[0], since)) return p;
	return picks.since(storage, p[0], since) ? [p[0], 0] : null;
}

/** Forgets every kept episode last written by a session one of
 *  `rows`' Continue cards opened — the rows are gone — and a
 *  recovery's pending point for its show, unless the episode's show
 *  is one of `keepShows`, still a remaining row's card: with `since`,
 *  only those written before it. */
export function clearRowPositions(
	rows: readonly string[],
	storage: PositionStorage | null = defaultStorage(),
	keepShows: ReadonlySet<string> = new Set(),
	since?: number
): void {
	const removed = new Set(rows);
	const kept: Positions = [];
	for (const p of load(storage)) {
		const show = p[0].slice(0, p[0].lastIndexOf(':'));
		const theRows = p[2] !== undefined && removed.has(p[2]) && !keepShows.has(show);
		const left = theRows ? afterRemoval(p, storage, since) : p;
		if (left !== p) recoveryResume.forgetShow(show, since);
		if (left) kept.push(left);
	}
	store(storage, kept);
}

/** Forgets every kept episode — the history is cleared — and any
 *  recovery's pending point. */
export function clearAllPositions(storage: PositionStorage | null = defaultStorage()): void {
	recoveryResume.forgetAll();
	store(storage, []);
}
