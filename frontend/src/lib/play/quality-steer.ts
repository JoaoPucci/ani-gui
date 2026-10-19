/**
 * A pin carried across levels replaced outright.
 *
 * A content-steering pathway switch replaces every level object, so
 * the remap keeps none of them and the hold re-seats at the setting's
 * level, letting the stream's pin go. The viewer's choice still
 * stands: the stream is pinned again at the run's pinned height (or
 * rank) among the new levels, when one is at or below it.
 */

import type { QualityGovernor } from './quality-governor';
import { pinnedLevel, type LadderLevel } from './quality-ladder';
import { qualityPin } from './quality-pin';

export function repinAfterReplacement(
	governor: QualityGovernor,
	mapping: readonly number[],
	wasPinned: boolean,
	levels: readonly LadderLevel[]
): void {
	const pin = qualityPin.choice();
	if (!wasPinned || pin === null || mapping.some((i) => i >= 0)) return;
	const level = pinnedLevel(pin, levels);
	if (level >= 0) governor.pick(level, 0);
}
