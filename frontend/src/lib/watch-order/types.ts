/**
 * The watch-order feature's data as the renderer draws it: the
 * backend's answers (`$lib/api`, "The watch order"), which decide
 * every order, number, grouping and progress, and the franchise entry
 * they carry. The modules beside this one only word and draw them.
 */

import type { FranchiseEntry } from '$lib/api';

/** One Kitsu anime as the franchise payload carries it ($lib/api's `FranchiseEntry`). */
export type WatchEntry = Readonly<FranchiseEntry>;
