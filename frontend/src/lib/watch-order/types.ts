/**
 * The watch-order feature's data, in the shape the backend's
 * `GET /api/kitsu/franchise/:id` serves it (docs/proposals/
 * franchise-watch-order.md, section 4): a run of entries joined by
 * prequel/sequel links, and the entries around it. Ordering, numbering
 * and every layout decision are made from these in the pure modules
 * beside this one.
 */

import type { Franchise, FranchiseEntry } from '$lib/api';

export type { Franchise, FranchiseEntry };

/** One Kitsu anime as the franchise payload carries it ($lib/api's `FranchiseEntry`). */
export type WatchEntry = Readonly<FranchiseEntry>;

/** A prequel → sequel pair between two run members. */
export type RunLink = readonly [string, string];

/**
 * The franchise payload read-only: $lib/api's `Franchise`, which every
 * reader here takes, so a fixture's read-only lists fit it too.
 */
export interface FranchiseData {
	/** The id asked about. */
	readonly anchor: string;
	/** The run, anchor included unless the anchor is a side story. */
	readonly entries: readonly WatchEntry[];
	readonly links: readonly RunLink[];
	readonly versions: readonly WatchEntry[];
	readonly films: readonly WatchEntry[];
	readonly sides: readonly WatchEntry[];
	readonly recaps: readonly WatchEntry[];
	/** A side-story anchor → its parent stories among `entries`. */
	readonly parents: Readonly<Record<string, readonly string[]>>;
	/** False when the walk stopped at its bound or a listing was cut short. */
	readonly complete: boolean;
}

/**
 * A run as the ordering works on it: the payload's sections plus the
 * entry the run is named after and numbered from (`nameId`).
 */
export interface WatchRun {
	/** The run's first entry: see `./name-id`. */
	readonly nameId: string;
	readonly entries: readonly WatchEntry[];
	readonly links: readonly RunLink[];
	readonly versions: readonly WatchEntry[];
	readonly films: readonly WatchEntry[];
	readonly sides: readonly WatchEntry[];
	readonly recaps: readonly WatchEntry[];
	readonly parents: Readonly<Record<string, readonly string[]>>;
}

/** An entry watched to its end, or the furthest episode played. */
export type EntryProgress = 'watched' | number;

/** Progress per Kitsu id; an entry with no row is absent. */
export type Progress = Readonly<Record<string, EntryProgress>>;
