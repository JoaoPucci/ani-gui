/**
 * The watch-order feature's data, in the shape the backend's
 * `GET /api/kitsu/franchise/:id` serves it (docs/title-resolution.md,
 * "Kitsu's relations, walked into a franchise"): a run of entries joined by
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
	/** The page's own chain, when the main story leaves the page out, or
	 *  its parent's, for a side story whose parents it leaves out. */
	readonly chain?: ChainData;
}

/** A chain outside the main story ($lib/api's `FranchiseChain`) read-only. */
export interface ChainData {
	/** Its own titles and its seams, start order. */
	readonly entries: readonly WatchEntry[];
	readonly links: readonly RunLink[];
	/** The ids of `entries` that are main-story titles. */
	readonly seams: readonly string[];
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
