/**
 * Which way a Strip can still scroll, and the wiring that keeps that
 * answer current. `Strip.svelte` is the adapter: it hands over its
 * scroller and rail elements and paints the edge fades and arrows
 * from what `onChange` reports.
 */

/** The three numbers the decision needs, read off the scroller. */
export interface ScrollMetrics {
	scrollLeft: number;
	clientWidth: number;
	scrollWidth: number;
}

export interface ScrollEdges {
	canScrollLeft: boolean;
	canScrollRight: boolean;
}

/** Sub-pixel layout and the rail's rounding leave a few pixels of
 *  slack at either end; an arrow for those would page nowhere. */
export const EDGE_SLACK_PX = 4;

export function scrollEdges(m: ScrollMetrics): ScrollEdges {
	return {
		canScrollLeft: m.scrollLeft > EDGE_SLACK_PX,
		canScrollRight: m.scrollLeft + m.clientWidth < m.scrollWidth - EDGE_SLACK_PX
	};
}

type ResizeObserverLike = { observe(el: Element): void; disconnect(): void };
type MutationObserverLike = {
	observe(el: Node, options: MutationObserverInit): void;
	disconnect(): void;
};

/** The observer constructors, injectable so the wiring runs under a
 *  test without a layout engine. Defaults to the browser's own. */
export interface OverflowObservers {
	ResizeObserver: new (cb: () => void) => ResizeObserverLike;
	MutationObserver: new (cb: () => void) => MutationObserverLike;
}

function browserObservers(): OverflowObservers {
	return {
		ResizeObserver: globalThis.ResizeObserver,
		MutationObserver: globalThis.MutationObserver
	};
}

/**
 * Report the scroller's edges now, and again whenever they may have
 * moved: on scroll, when the scroller's box resizes, and when the
 * content inside it changes. The last is the one a box-only watch
 * misses — cards added, removed or collapsed change `scrollWidth`
 * while the scroller keeps its size — so the rail's child list is
 * watched, and each card's own box. Returns the teardown.
 */
export function watchStripOverflow(
	scroller: HTMLElement,
	content: HTMLElement,
	onChange: (edges: ScrollEdges) => void,
	observers: OverflowObservers = browserObservers()
): () => void {
	const update = () => onChange(scrollEdges(scroller));
	scroller.addEventListener('scroll', update, { passive: true });

	const ro = new observers.ResizeObserver(update);
	const observeBoxes = () => {
		ro.observe(scroller);
		for (const child of Array.from(content.children)) ro.observe(child);
	};
	observeBoxes();

	// A removed card cannot report its own departure, so the child
	// list is what tells the strip its content changed. The resize
	// observer is re-pointed at the current cards so a departed one is
	// not held on to.
	const mo = new observers.MutationObserver(() => {
		ro.disconnect();
		observeBoxes();
		update();
	});
	mo.observe(content, { childList: true });

	update();
	return () => {
		scroller.removeEventListener('scroll', update);
		ro.disconnect();
		mo.disconnect();
	};
}
