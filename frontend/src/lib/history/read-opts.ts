// The options argument the Continue resolver hands each read.

import type { ReadOptions } from '$lib/api';

/** The read's options: the signal when the resolution was given one,
 *  and no argument at all otherwise, so a read asked without a signal
 *  is asked exactly as before. */
export function readOpts(opts: ReadOptions): [] | [ReadOptions] {
	return opts.signal ? [{ signal: opts.signal }] : [];
}
