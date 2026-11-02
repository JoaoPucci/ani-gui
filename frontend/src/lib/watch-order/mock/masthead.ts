/**
 * The franchise page's masthead (mockup): the entry whose artwork it
 * shows. Part of the mockups' views.
 */
import type { WatchOrder } from './by-start';
import { currentIn } from './handoff';
import type { MockEntry, MockRun } from './types';

/** The entry the masthead's backdrop and poster come from. */
export function mastheadArt(run: MockRun, order: WatchOrder): MockEntry {
	const first = run.entries.find((e) => e.id === run.nameId)!;
	return currentIn(run, order)?.entry ?? first;
}
