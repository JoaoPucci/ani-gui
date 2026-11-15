/**
 * The franchise page's masthead (mockup): the entry whose artwork it
 * shows. Part of the mockups' views.
 */
import type { WatchOrder } from '../by-start';
import type { MockEntry, MockRun } from './types';

/**
 * The entry the masthead's backdrop and poster come from: the one the
 * franchise is named after, so the artwork beside the name is that
 * entry's. The entry being watched has its own line under the Resume
 * button. `order` does not change it: the name, and so the artwork,
 * is the entry the franchise is named after (the run's first in link
 * order), whatever the order setting.
 */
export function mastheadArt(run: MockRun, order: WatchOrder): MockEntry {
	void order;
	return run.entries.find((e) => e.id === run.nameId)!;
}
