/**
 * Shapes of the watch-order mockups' fixture data. A run is the
 * entries one franchise's prequel/sequel links join (the proposal's
 * `chain`); the other lists are the entries around it (`around`).
 */

export interface MockEntry {
	readonly id: string;
	readonly title: string;
	/** Kitsu's subtype as served: `TV`, `movie`, `special`, `OVA`, `ONA`. */
	readonly subtype: string;
	/** `YYYY-MM-DD`, or null when Kitsu has no date. */
	readonly start: string | null;
	readonly episodes: number | null;
	/** Kitsu's status: `finished`, `current`, `upcoming`, `tba`. */
	readonly status: string;
	readonly poster: string | null;
	readonly cover: string | null;
}

/** `watched`, or the last episode played. */
export type MockProgress = 'watched' | number;

export interface MockRun {
	readonly key: string;
	/** The entry the franchise is named after: the run's first. */
	readonly nameId: string;
	readonly entries: readonly MockEntry[];
	readonly versions: readonly MockEntry[];
	readonly films: readonly MockEntry[];
	readonly sides: readonly MockEntry[];
	readonly recaps: readonly MockEntry[];
	/** Prequel → sequel pairs between members, from either listing. */
	readonly links: readonly (readonly [string, string])[];
	readonly progress: Readonly<Record<string, MockProgress>>;
}
