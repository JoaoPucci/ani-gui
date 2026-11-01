// Which Watch Later cards move when the row is read again after a
// tracker write settles, and the gate the row's transitions read at
// fire time. The motion itself is the Continue Watching strip's
// (`$lib/transitions/card-motion`), gated the same way
// (`$lib/history/animation-gate`): only what a settled write changed
// moves; a row that fills or re-sorts on load does not.

import { createAnimationGate, type AnimationGateDeps } from '$lib/history/animation-gate';

/** What changed between two renders of the row, by card id. */
export interface RailDelta {
	/** Cards the new render has that the old did not. */
	added: string[];
	/** Cards the old render had that the new does not. */
	removed: string[];
	/** Cards in both whose place changed. */
	shifted: string[];
}

export function railDelta(before: readonly string[], after: readonly string[]): RailDelta {
	const was = new Map(before.map((id, i) => [id, i]));
	const now = new Set(after);
	return {
		added: after.filter((id) => !was.has(id)),
		removed: before.filter((id) => !now.has(id)),
		shifted: after.filter((id, i) => was.has(id) && was.get(id) !== i)
	};
}

/** Read by the row's `in:`, `out:` and `animate:` at fire time. */
export interface RailMotionGate {
	open(delta: RailDelta): void;
	arrives(id: string): boolean;
	leaves(id: string): boolean;
	shifts(id: string): boolean;
}

/** Two of the Continue Watching strip's gates: one for the cards
 *  leaving and the ones sliding beside them, one for the cards arriving.
 *  Each closes on its own timer, so a load after it moves nothing. */
export function createRailMotionGate(holdMs?: number, deps?: AnimationGateDeps): RailMotionGate {
	const leaving = createAnimationGate(holdMs, deps);
	const arriving = createAnimationGate(holdMs, deps);
	return {
		open(delta) {
			leaving.open(delta.removed, delta.shifted);
			arriving.open(delta.added, []);
		},
		arrives: (id) => arriving.shouldAnimateRemoval(id),
		leaves: (id) => leaving.shouldAnimateRemoval(id),
		shifts: (id) => leaving.shouldAnimateShift(id)
	};
}
