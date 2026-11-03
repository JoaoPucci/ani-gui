/**
 * The watch-order feature's data, in the shape the backend's
 * `GET /api/kitsu/franchise/:id` serves it (docs/proposals/
 * franchise-watch-order.md, section 4): a run of entries joined by
 * prequel/sequel links, and the entries around it. Ordering, numbering
 * and every layout decision are made from these in the pure modules
 * beside this one.
 */

/** One Kitsu anime as the franchise payload carries it. */
export interface WatchEntry {
	readonly id: string;
	/** Kitsu's canonical title. */
	readonly title: string;
	/** Kitsu's subtype as served (`TV`, `movie`, `OVA`, `ONA`, `special`), or null. */
	readonly subtype: string | null;
	/** `YYYY-MM-DD`, or null when Kitsu has no date. */
	readonly start: string | null;
	readonly episodes: number | null;
	/** Kitsu's status (`finished`, `current`, `upcoming`, `tba`), or null. */
	readonly status: string | null;
	readonly poster: string | null;
	readonly cover: string | null;
}

/** A prequel → sequel pair between two run members. */
export type RunLink = readonly [string, string];

/** The franchise payload: the run, the entries around it, and a side story's parents. */
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
