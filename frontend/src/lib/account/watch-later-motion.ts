// Which Watch Later cards move when the row is read again after a
// tracker write settles, and the gate the row's transitions read at
// fire time. The motion itself is the Continue Watching strip's
// (`$lib/transitions/card-motion`), gated the same way
// (`$lib/history/animation-gate`): only what a settled write changed
// moves; a row that fills or re-sorts on load does not.

import type { AnimationGateDeps } from '$lib/history/animation-gate';

/** What changed between two renders of the row, by card id. */
export interface RailDelta {
	/** Cards the new render has that the old did not. */
	added: string[];
	/** Cards the old render had that the new does not. */
	removed: string[];
	/** Cards in both whose place changed. */
	shifted: string[];
}

export const railDelta: (
	before: readonly string[],
	after: readonly string[]
) => RailDelta = () => ({
	added: [],
	removed: [],
	shifted: []
});

/** Read by the row's `in:`, `out:` and `animate:` at fire time. */
export interface RailMotionGate {
	open(delta: RailDelta): void;
	arrives(id: string): boolean;
	leaves(id: string): boolean;
	shifts(id: string): boolean;
}

export const createRailMotionGate: (
	holdMs?: number,
	deps?: AnimationGateDeps
) => RailMotionGate = () => ({
	open: () => {},
	arrives: () => false,
	leaves: () => false,
	shifts: () => false
});
