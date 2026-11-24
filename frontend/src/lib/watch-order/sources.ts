/**
 * The pages' sources for the watch-order reads: the backend's routes,
 * each sent with the viewer's local day ("Coming …" is judged against
 * it) and cancelled by the visit's signal.
 */
import { watchOrderFranchise, watchOrderShow, watchOrderUpNext } from '$lib/api';
import { localDay } from './entry';
import type { WatchOrderSources } from './loader';

const today = () => localDay(new Date());

export const watchOrderSources: WatchOrderSources = {
	show: (id, signal) => watchOrderShow(id, { today: today() }, { signal }),
	franchise: (id, signal) => watchOrderFranchise(id, { today: today() }, { signal }),
	upNext: (id, episode, signal) => watchOrderUpNext(id, { today: today(), episode }, { signal })
};
