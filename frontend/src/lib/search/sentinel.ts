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
) => Pick<IntersectionObserver, 'observe' | 'disconnect'>;

/** Start watching `node`; `onChange` hears every change of whether it
 *  is in reach. Returns the function that stops watching. Without an
 *  observer the marker never reports in reach — nothing auto-loads,
 *  and the grid is the first page alone. */
export function watchSentinel(
	node: Element,
	onChange: (inReach: boolean) => void,
	Observer: ObserverCtor | undefined = globalThis.IntersectionObserver
): () => void {
	if (!Observer) {
		onChange(false);
		return () => {};
	}
	const observer = new Observer(
		// Entries arrive oldest first; a batch is never empty.
		(entries) => onChange(entries[entries.length - 1].isIntersecting),
		{ root: null, rootMargin: SENTINEL_MARGIN }
	);
	observer.observe(node);
	return () => observer.disconnect();
}
