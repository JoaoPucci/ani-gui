// The per-card episode wait the Continue strip runs: one timer per
// card, started when the card's match lands; see ./card-look for what
// the wait running out does.

export interface CapTimerDeps {
	capMs: number;
	schedule: (fn: () => void, ms: number) => unknown;
	cancel: (handle: unknown) => void;
	onElapsed: (entryId: string) => void;
}

/**
 * One episode-wait timer per card, started when the card's match
 * lands. Arming a card twice keeps the first timer: the wait counts
 * from the match, not from the latest re-render.
 */
export function createCapTimers(deps: CapTimerDeps) {
	const handles = new Map<string, unknown>();
	let disposed = false;
	return {
		arm(entryId: string) {
			if (disposed || handles.has(entryId)) return;
			handles.set(
				entryId,
				deps.schedule(() => deps.onElapsed(entryId), deps.capMs)
			);
		},
		dispose() {
			disposed = true;
			for (const h of handles.values()) deps.cancel(h);
			handles.clear();
		}
	};
}
