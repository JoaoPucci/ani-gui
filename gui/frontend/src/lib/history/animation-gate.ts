/**
 * Open/auto-close gate the Continue Watching row uses to decide
 * whether out:scale + animate:flip should play a real transition
 * or short-circuit to duration 0. Scoped per-id so a concurrent
 * `dedupeHistoryByKitsuId` mutation during the open window can't
 * piggyback on the gate and reintroduce the flicker — only ids
 * the caller passed to `open()` animate.
 *
 * Three non-obvious bits this helper exists to test:
 *
 *  1. `dedupeHistoryByKitsuId` collapses sibling rows as their
 *     Kitsu matches resolve on home page mount. Without a gate,
 *     every dedupe-driven removal fires out:scale and every
 *     survivor fires animate:flip — the resize-flicker users see
 *     on Home → Detail → back. The gate is empty during load;
 *     `shouldAnimate(id)` returns false for every dedupe row.
 *
 *  2. Svelte 5 batches the `history = ...` and `deleteBusy =
 *     false` assignments in `confirmDelete`'s finally block.
 *     By the time Svelte runs the out-transition factory, both
 *     have been applied — gating on `deleteBusy` reads false
 *     and skips the animation the user just triggered. The gate
 *     auto-closes on a timer that fires AFTER Svelte's
 *     synchronous batch processes, so the factory still reads
 *     `shouldAnimate(id) === true` at fire time.
 *
 *  3. While the timer is open (typically 350ms), a background
 *     `loadContinueWatchingState` callback can resolve another
 *     row's Kitsu match and trigger a dedupe-removal of a
 *     *different* row. With a boolean gate, that unrelated
 *     removal would also animate. With per-id scope, only the
 *     ids the user's delete actually touched animate — the
 *     concurrent dedupe rows are absent from the set and
 *     short-circuit to duration 0.
 *
 * `holdMs` defaults to 350ms — the longest transition this gate
 * guards is the 280ms `animate:flip` + 70ms margin. Tune via the
 * factory arg if the transition durations change.
 */
export interface AnimationGate {
	/** Mark a specific set of ids as "should animate"; auto-closes
	 *  after `holdMs`. Idempotent — calling open() again replaces
	 *  the set and resets the timer. */
	open(ids: Iterable<string>): void;
	/** Read at transition fire time. */
	shouldAnimate(id: string): boolean;
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
	let active = new Set<string>();
	let timer: unknown = null;
	return {
		open(ids: Iterable<string>) {
			active = new Set(ids);
			if (timer !== null) deps.clearTimeout(timer);
			timer = deps.setTimeout(() => {
				active = new Set();
				timer = null;
			}, holdMs);
		},
		shouldAnimate(id: string) {
			return active.has(id);
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
 * survivors AFTER the first removed index move left. We return a
 * combined list of removed ids + shifted survivor ids so the
 * caller can hand the whole set to `open()`.
 */
export function idsAffectedByDelete(
	snapshot: readonly string[],
	removedIds: readonly string[]
): string[] {
	if (removedIds.length === 0) return [];
	const removedSet = new Set(removedIds);
	const firstRemovedIdx = snapshot.findIndex((id) => removedSet.has(id));
	if (firstRemovedIdx < 0) return [...removedIds];
	const shiftedSurvivors = snapshot.slice(firstRemovedIdx + 1).filter((id) => !removedSet.has(id));
	return [...removedIds, ...shiftedSurvivors];
}
