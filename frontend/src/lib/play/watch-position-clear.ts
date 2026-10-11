/**
 * Forgetting kept positions: one episode, a removed show's, the
 * episodes a removed history row's Continue cards wrote, or all of
 * them. Split from watch-position.ts so each file stays inside the
 * CRAP gate's per-file bar; watch-position.ts re-exports all of it.
 */

import { recoveryResume } from './resume-after-recovery';
import { nextWrite, picks, writes } from './write-order';
import {
	defaultStorage,
	keyOf,
	load,
	store,
	type Position,
	type PositionStorage,
	type Positions
} from './watch-position-storage';

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
