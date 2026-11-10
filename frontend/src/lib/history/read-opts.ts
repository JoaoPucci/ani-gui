// The options argument the Continue resolver hands each read. Not
// wired yet: no read takes one.

import type { ReadOptions } from '$lib/api';

/** The read's options; none yet. */
export function readOpts(opts: ReadOptions): [] | [ReadOptions] {
	void opts;
	return [];
}
