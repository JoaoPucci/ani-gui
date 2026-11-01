// The episode wait the Continue strip runs: one timer per card and
// offered episode, started when the card first offers that episode;
// see ./card-look for what the wait running out does.

export interface CapTimerDeps {
	capMs: number;
	schedule: (fn: () => void, ms: number) => unknown;
	cancel: (handle: unknown) => void;
	onElapsed: (key: string) => void;
}

/**
 * One episode-wait timer per key (a card and the episode it offers).
 * Arming a key twice keeps the first timer: the wait counts from when
 * the card first offered that episode, not from the latest re-render.
 * `arm` answers whether a wait is running or has run for the key, so
 * the page can arm as it reads; after `dispose` it arms nothing and
 * answers false.
 */
export function createCapTimers(deps: CapTimerDeps) {
	const handles = new Map<string, unknown>();
	let disposed = false;
	return {
		arm(key: string): boolean {
			if (disposed) return false;
			if (!handles.has(key)) {
				handles.set(
					key,
					deps.schedule(() => deps.onElapsed(key), deps.capMs)
				);
			}
			return true;
		},
		dispose() {
			disposed = true;
			for (const h of handles.values()) deps.cancel(h);
			handles.clear();
		}
	};
}
