// The episode wait the Continue strip runs: one timer per card and
// offered episode, started when the card first offers that episode;
// see ./card-look for what the wait running out does.

export interface CapTimerDeps {
	capMs: number;
	schedule: (fn: () => void, ms: number) => unknown;
	cancel: (handle: unknown) => void;
	onElapsed: (key: string) => void;
	/** How the warm still out for a show ends, or null when none is
	 *  ($lib/history/strip-warm `createWarmWatch`). */
	pendingWarm?: (show: string) => Promise<'answered' | 'failed'> | null;
	/** The longest a card waits, from its first arm, while a warm for
	 *  its show is out. */
	ceilingMs?: number;
}

/**
 * One episode-wait timer per key (a card and the episode it offers).
 * Arming a key twice keeps the first timer: the wait counts from when
 * the card first offered that episode, not from the latest re-render.
 * `arm` answers whether a wait is running or has run for the key, so
 * the page can arm as it reads; after `dispose` it arms nothing and
 * answers false.
 *
 * A card armed with its `show` waits past the cap while Home's strip
 * warm for that show is still out (`pendingWarm`): its own episode read
 * and the warm share one AniList lookup for the show, so its details
 * come back by the time that lookup answers, or once the read's own
 * two-second AniList budget runs out. It waits on, to `ceilingMs` from
 * its first arm, for those details; a warm that fails or is dropped
 * past the cap ends the wait at once. A card no warm is out for at the
 * cap, its warm having ended or never carried the show, runs the plain
 * cap. The show is the one the card offered when first armed.
 */
export function createCapTimers(deps: CapTimerDeps) {
	const handles = new Map<string, unknown>();
	let disposed = false;
	const elapse = (key: string) => {
		if (!disposed) deps.onElapsed(key);
	};
	// The cap ran out with a warm still out for the show.
	const holdFor = (key: string, warm: Promise<'answered' | 'failed'>) => {
		const rest = Math.max(0, (deps.ceilingMs ?? deps.capMs) - deps.capMs);
		let done = false;
		const end = () => {
			if (done) return;
			done = true;
			elapse(key);
		};
		handles.set(key, deps.schedule(end, rest));
		void warm.then((outcome) => {
			if (outcome === 'failed' && !disposed) {
				deps.cancel(handles.get(key));
				end();
			}
		});
	};
	return {
		arm(key: string, show?: string): boolean {
			if (disposed) return false;
			if (!handles.has(key)) {
				handles.set(
					key,
					deps.schedule(() => {
						const warm = show === undefined ? null : (deps.pendingWarm?.(show) ?? null);
						if (warm) holdFor(key, warm);
						else elapse(key);
					}, deps.capMs)
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
