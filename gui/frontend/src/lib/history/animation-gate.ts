/**
 * Open/auto-close gate the Continue Watching row uses to decide
 * whether out:scale + animate:flip should play a real transition
 * or short-circuit to duration 0. Scoped per-id AND split per
 * transition kind:
 *
 *   - `shouldAnimateRemoval(id)`: drives `out:scale`. True only
 *     for the ids the user's confirm actually deleted. A
 *     shifted-survivor that gets dedupe-removed by a background
 *     callback during the window MUST NOT animate (Codex P2
 *     #3369281412) — otherwise the flicker creeps back in via
 *     the survivor's out-transition.
 *
 *   - `shouldAnimateShift(id)`: drives `animate:flip`. True for
 *     the ids that physically shift left to close the gap —
 *     i.e., the survivors positioned after the first removed
 *     row in the visible order at delete time. These are the
 *     only nodes whose `animate:flip` should actually play; any
 *     other flip happening during the window is a concurrent
 *     dedupe-driven reposition and should pass through instantly.
 *
 * Two non-obvious bits:
 *
 *  1. `dedupeHistoryByKitsuId` collapses sibling rows as their
 *     Kitsu matches resolve on home page mount. Without the
 *     gate, every dedupe-driven removal fires out:scale and
 *     every survivor fires animate:flip — the resize-flicker
 *     users see on Home → Detail → back. Both sets are empty
 *     during load; `shouldAnimate*` returns false everywhere.
 *
 *  2. Svelte 5 batches the `history = ...` and `deleteBusy =
 *     false` assignments in `confirmDelete`'s finally block.
 *     By the time Svelte runs the out-transition factory, both
 *     have been applied — gating on `deleteBusy` reads false
 *     and skips the animation the user just triggered. The gate
 *     auto-closes on a timer that fires AFTER Svelte's
 *     synchronous batch processes, so the factory still reads
 *     `shouldAnimate*(id) === true` at fire time.
 *
 * `holdMs` defaults to 350ms — the longest transition this gate
 * guards is the 280ms `animate:flip` + 70ms margin.
 */
export interface AnimationGate {
	/** Set the two ids buckets and start the auto-close timer.
	 *  Idempotent — calling open again replaces both sets and
	 *  resets the timer. */
	open(removedIds: Iterable<string>, shiftedIds: Iterable<string>): void;
	/** Read by `cwOutScale` at transition fire time. */
	shouldAnimateRemoval(id: string): boolean;
	/** Read by `cwFlip` at transition fire time. */
	shouldAnimateShift(id: string): boolean;
}

export interface AnimationGateDeps {
	setTimeout: (cb: () => void, ms: number) => unknown;
	clearTimeout: (handle: unknown) => void;
}

const defaultDeps: AnimationGateDeps = {
	setTimeout: (cb, ms) => setTimeout(cb, ms),
	clearTimeout: (h) => clearTimeout(h as Parameters<typeof clearTimeout>[0])
};

export function createAnimationGate(
	holdMs = 350,
	deps: AnimationGateDeps = defaultDeps
): AnimationGate {
	let removed = new Set<string>();
	let shifted = new Set<string>();
	let timer: unknown = null;
	return {
		open(removedIds: Iterable<string>, shiftedIds: Iterable<string>) {
			removed = new Set(removedIds);
			shifted = new Set(shiftedIds);
			if (timer !== null) deps.clearTimeout(timer);
			timer = deps.setTimeout(() => {
				removed = new Set();
				shifted = new Set();
				timer = null;
			}, holdMs);
		},
		shouldAnimateRemoval(id: string) {
			return removed.has(id);
		},
		shouldAnimateShift(id: string) {
			return shifted.has(id);
		}
	};
}

/**
 * Given the snapshot of dedupedHistory ids BEFORE the delete and
 * the ids being removed by it, compute which surviving ids will
 * physically shift position when the deletion lands — i.e., the
 * ones `animate:flip` should animate to slide-in-to-close-the-gap.
 *
 * Survivors that sit BEFORE every removed row don't shift; only
 * survivors AFTER the first removed index move left.
 */
export function shiftedSurvivorIds(
	snapshot: readonly string[],
	removedIds: readonly string[]
): string[] {
	if (removedIds.length === 0) return [];
	const removedSet = new Set(removedIds);
	const firstRemovedIdx = snapshot.findIndex((id) => removedSet.has(id));
	if (firstRemovedIdx < 0) return [];
	return snapshot.slice(firstRemovedIdx + 1).filter((id) => !removedSet.has(id));
}
