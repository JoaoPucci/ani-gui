/**
 * The move a choice in the player's quality menu makes on the engine.
 */

import type Hls from 'hls.js';
import type { QualityGovernor, QualityMove } from './quality-governor';
import type { QualityTrack } from './quality-track';

/** The switch a choice already in force still needs: the level held,
 *  when a segment of another level is what plays; null otherwise. */
export function staleChoice(
	governor: QualityGovernor,
	choice: 'auto' | number,
	playing: number | null
): QualityMove | null {
	const held = governor.level;
	const inForce = choice === 'auto' ? !governor.isPinned : governor.pinnedLevel === choice;
	if (!inForce || playing === null || playing === held) return null;
	return { level: held, from: playing, reason: choice === 'auto' ? 'auto' : 'pick', via: 'next' };
}

/** The move a menu choice makes: the governor's own; or the level held
 *  again when a lower one still plays from the buffer; or, when the
 *  engine was left choosing (the control had been released), the level
 *  held, so the choice the menu shows is the one the engine plays. */
export function pickMove(
	governor: QualityGovernor,
	choice: 'auto' | number,
	now: number,
	track: QualityTrack,
	engine: Pick<Hls, 'autoLevelEnabled'>
): QualityMove | null {
	const move = governor.pick(choice, now) ?? staleChoice(governor, choice, track.playingLevel);
	if (move || !governor.isEngaged || !engine.autoLevelEnabled) return move;
	const reason = choice === 'auto' ? 'auto' : 'pick';
	return { level: governor.level, from: governor.level, reason, via: 'next' };
}

/** The governor a menu choice goes to; null with no stream, or with a
 *  stream of one level, which has nothing to choose — its menu entry
 *  must not re-pin the run on that level either. */
export function choosable(
	governor: QualityGovernor | null,
	track: QualityTrack
): QualityGovernor | null {
	return governor && track.declared().length > 1 ? governor : null;
}
