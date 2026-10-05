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

/** The observer constructors, injectable so the wiring runs under a
 *  test without a layout engine. Defaults to the browser's own. */
export interface OverflowObservers {
	ResizeObserver: new (cb: () => void) => ResizeObserverLike;
}

function browserObservers(): OverflowObservers {
	return { ResizeObserver: globalThis.ResizeObserver };
}

/**
 * Report the scroller's edges now, and again whenever they may have
 * moved. Returns the teardown.
 */
export function watchStripOverflow(
	scroller: HTMLElement,
	_content: HTMLElement,
	onChange: (edges: ScrollEdges) => void,
	observers: OverflowObservers = browserObservers()
): () => void {
	const update = () => onChange(scrollEdges(scroller));
	update();
	scroller.addEventListener('scroll', update, { passive: true });
	const ro = new observers.ResizeObserver(update);
	ro.observe(scroller);
	return () => {
		scroller.removeEventListener('scroll', update);
		ro.disconnect();
	};
}
