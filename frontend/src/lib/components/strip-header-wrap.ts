/**
 * Whether a row header's status starts a line of its own, and the
 * wiring that keeps that answer current. The header is a wrapping
 * flex line (Strip.svelte); its status opens with a "·" that separates
 * it from the caption. When a long translation pushes the status onto
 * the next line, that dot would open the line and separate nothing, so
 * Strip hides it while this reports a wrap.
 */

/** The two edges the decision needs, read off an element's box. */
export interface LineBox {
	top: number;
	bottom: number;
}

/** True when `item` sits on a later line than `lead`: its vertical
 *  centre falls below the bottom of `lead`'s box. */
export function startsNewLine(lead: LineBox, item: LineBox): boolean {
	return (item.top + item.bottom) / 2 > lead.bottom;
}

/** What the header last decided, and at which width. */
export interface WrapState {
	wraps: boolean;
	width: number;
}

/**
 * The next wrap decision, given a fresh measurement at `width`.
 *
 * Hiding the separator narrows the status, which can let it fit back
 * on the title's line; measured again, it would no longer wrap, the
 * separator would return, and it would wrap again — a loop on every
 * header resize at widths near the edge. So a wrap holds until the
 * header grows wider than it was when the wrap was decided: narrowing
 * or a same-width resize cannot undo it.
 */
export function holdWrap(prev: WrapState | null, measured: boolean, width: number): WrapState {
	void prev;
	void measured;
	void width;
	throw new Error('not implemented');
}

type ResizeObserverLike = { observe(el: Element): void; disconnect(): void };

/**
 * Report whether `item` starts a new line after `lead` now, and again
 * whenever the header's box resizes (a window resize, a translation
 * swapped in, a status that grew). Returns the teardown.
 */
export function watchHeaderWrap(
	header: Element,
	lead: Element,
	item: Element,
	onChange: (wraps: boolean) => void,
	Observer: new (cb: () => void) => ResizeObserverLike = globalThis.ResizeObserver
): () => void {
	const update = () =>
		onChange(startsNewLine(lead.getBoundingClientRect(), item.getBoundingClientRect()));
	const ro = new Observer(update);
	ro.observe(header);
	update();
	return () => ro.disconnect();
}
