/**
 * Watches the marker at the bottom of the search grid and reports
 * whether it is near the viewport, so the next page can start loading
 * before the reader reaches the last row. The page scrolls the
 * document, so the viewport is the observer's root and the margin
 * extends it downward.
 */

/** How far below the viewport the marker counts as reached: about a
 *  screen of posters, so a page lands before the reader runs out. */
export const SENTINEL_MARGIN = '0px 0px 800px 0px';

type ObserverCtor = new (
	cb: IntersectionObserverCallback,
	init?: IntersectionObserverInit
) => Pick<IntersectionObserver, 'observe' | 'unobserve' | 'disconnect'>;

/** A watched marker: stop watching it, or ask for a new measurement. */
export interface SentinelWatch {
	stop: () => void;
	/** Measure again: the observer reports the marker's state against
	 *  the layout as it stands now — after a page has landed, say —
	 *  whether or not it changed. */
	remeasure: () => void;
}

/** Start watching `node`; `onChange` hears every measurement of
 *  whether it is in reach. Without an observer the marker never
 *  reports in reach — nothing auto-loads, and the grid is the first
 *  page alone. */
export function watchSentinel(
	node: Element,
	onChange: (inReach: boolean) => void,
	Observer: ObserverCtor | undefined = globalThis.IntersectionObserver,
	now: () => number = () => performance.now()
): SentinelWatch {
	if (!Observer) {
		onChange(false);
		return { stop: () => {}, remeasure: () => {} };
	}
	// Entries measured before the last remeasure describe a layout that
	// has since changed; `unobserve` does not drop ones already queued.
	let since = -Infinity;
	const observer = new Observer(
		(entries) => {
			// Entries arrive oldest first, so the last current one is the
			// newest measurement. Written as "not older than" so that,
			// before any remeasure, every entry counts.
			const current = entries.filter((e) => !(e.time < since));
			if (current.length > 0) onChange(current[current.length - 1].isIntersecting);
		},
		{ root: null, rootMargin: SENTINEL_MARGIN }
	);
	observer.observe(node);
	return {
		stop: () => observer.disconnect(),
		remeasure: () => {
			since = now();
			observer.unobserve(node);
			observer.observe(node);
		}
	};
}
