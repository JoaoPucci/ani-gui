/**
 * How a row header's line breaks, and the wiring that keeps that
 * answer current. The header is a wrapping flex line (Strip.svelte):
 * title, then an optional status opened by a "·", then the actions.
 *
 *   - `metaWraps`: the status does not fit after the title. Strip then
 *     hides the "·" (it would open a line and separate nothing) and
 *     gives the title the whole first line.
 *   - `actionsWrap`: the actions sit below the title's line. On a
 *     banded row they are pushed to the band's end; once they wrap that
 *     push would strand them at the far end of an otherwise empty line,
 *     so Strip drops it.
 *
 * The status decision is made from natural widths — the title's
 * content, the separator, the status — against the room the header
 * has, never from where the browser has put the status. Strip changes
 * the layout in answer to the decision (the hidden separator, the
 * title's full line), and a decision read off that layout would only
 * ever confirm itself: a wrapped status would never come back when the
 * window widened.
 */

/** The two edges a line test needs, read off an element's box. */
export interface LineBox {
	top: number;
	bottom: number;
}

/** Horizontal extent of a box. */
export interface Span {
	left: number;
	right: number;
}

/** True when `item` sits on a later line than `lead`: its vertical
 *  centre falls below the bottom of `lead`'s box. */
export function startsNewLine(lead: LineBox, item: LineBox): boolean {
	return (item.top + item.bottom) / 2 > lead.bottom;
}

/** The width from the leftmost to the rightmost of `boxes`; 0 for none.
 *  Direction-free, so it measures the same in a right-to-left line. */
export function naturalSpan(boxes: readonly Span[]): number {
	if (boxes.length === 0) return 0;
	return Math.max(...boxes.map((b) => b.right)) - Math.min(...boxes.map((b) => b.left));
}

/** The widths the status decision compares. */
export interface StatusFit {
	/** The header's content width: what one line can hold. */
	available: number;
	/** The title's content: key, rule, caption. */
	title: number;
	/** The separator, measured while shown. */
	sep: number;
	/** The status words. */
	status: number;
	/** The header's gap between items. */
	gap: number;
}

/** True when title, separator and status do not fit on one line. */
export function statusWraps(fit: StatusFit): boolean {
	return fit.title + fit.gap + fit.sep + fit.gap + fit.status > fit.available;
}

export interface HeaderLines {
	metaWraps: boolean;
	actionsWrap: boolean;
}

/** The header's parts, as Strip binds them. */
export interface HeaderParts {
	header: Element;
	title: Element;
	sep: Element | null;
	status: Element | null;
	actions: Element | null;
}

type ResizeObserverLike = { observe(el: Element): void; disconnect(): void };

/** What reading the header needs from the page, injectable so the
 *  wiring runs under a test without a layout engine. */
export interface HeaderDeps {
	Observer: new (cb: () => void) => ResizeObserverLike;
	/** The header's padding at either inline edge and its column gap, in px. */
	styleOf: (header: Element) => { padStart: number; padEnd: number; gap: number };
}

/**
 * Report the header's lines now, and again whenever the header or one
 * of its parts resizes (a window resize, a translation swapped in, a
 * status that changed its words). Returns the teardown.
 */
export function watchHeaderLines(
	parts: HeaderParts,
	onChange: (lines: HeaderLines) => void,
	deps: HeaderDeps
): () => void {
	const { header, title, sep, status, actions } = parts;
	// The separator is hidden while the status wraps; its width is the
	// one it had when last shown, so hiding it cannot change the answer.
	let sepWidth = 0;
	const update = () => {
		const { padStart, padEnd, gap } = deps.styleOf(header);
		const shown = sep?.getBoundingClientRect().width ?? 0;
		if (shown > 0) sepWidth = shown;
		const metaWraps =
			status !== null &&
			statusWraps({
				available: header.clientWidth - padStart - padEnd,
				title: naturalSpan(Array.from(title.children, (c) => c.getBoundingClientRect())),
				sep: sepWidth,
				status: status.getBoundingClientRect().width,
				gap
			});
		const actionsWrap =
			actions !== null &&
			startsNewLine(title.getBoundingClientRect(), actions.getBoundingClientRect());
		onChange({ metaWraps, actionsWrap });
	};
	const ro = new deps.Observer(update);
	for (const el of [header, title, status, actions]) if (el) ro.observe(el);
	update();
	return () => ro.disconnect();
}
