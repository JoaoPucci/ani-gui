/**
 * The pages' sources for `loadWatchOrder`: the franchise route, and
 * the history rows the franchise's entries resume from, each cancelled
 * by the visit's signal.
 */
import { historyByKitsuEach, kitsuFranchise } from '$lib/api';
import type { WatchOrderSources } from './loader';

export const watchOrderSources: WatchOrderSources = {
	franchise: (id, signal) => kitsuFranchise(id, { signal }),
	named: (ids, signal) => historyByKitsuEach(ids, { signal })
};
