/**
 * The pages' sources for `loadWatchOrder`: the franchise route, and
 * the history rows the franchise's entries resume from.
 */
import { historyByKitsuEach, kitsuFranchise } from '$lib/api';
import type { WatchOrderSources } from './loader';

export const watchOrderSources: WatchOrderSources = {
	franchise: kitsuFranchise,
	named: historyByKitsuEach
};
