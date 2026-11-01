import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	naturalSpan,
	startsNewLine,
	statusWraps,
	watchHeaderLines,
	type HeaderDeps,
	type LineBox
} from './strip-header-wrap';

const TITLE: LineBox = { top: 100, bottom: 116 };

describe('startsNewLine', () => {
	it('is false for an item level with the title', () => {
		expect(startsNewLine(TITLE, { top: 92, bottom: 124 })).toBe(false);
		expect(startsNewLine(TITLE, { top: 101, bottom: 115 })).toBe(false);
	});

	it('is true for an item on the next line', () => {
		expect(startsNewLine(TITLE, { top: 124, bottom: 156 })).toBe(true);
	});

	it('is false for an item following the caption onto a wrapped title’s second line', () => {
		expect(startsNewLine({ top: 100, bottom: 150 }, { top: 130, bottom: 150 })).toBe(false);
	});
});

describe('startsNewLine, by property', () => {
	const box = fc
		.tuple(fc.integer({ min: 0, max: 2000 }), fc.integer({ min: 1, max: 80 }))
		.map(([top, h]) => ({ top, bottom: top + h }));

	it('is true for anything wholly below the lead', () => {
		fc.assert(
			fc.property(
				box,
				fc.integer({ min: 0, max: 200 }),
				fc.integer({ min: 1, max: 80 }),
				(lead, gap, h) => {
					expect(
						startsNewLine(lead, { top: lead.bottom + gap, bottom: lead.bottom + gap + h })
					).toBe(true);
				}
			)
		);
	});

	it('is false for anything centred within the lead’s span', () => {
		fc.assert(
			fc.property(box, fc.integer({ min: 0, max: 60 }), (lead, reach) => {
				const centre = (lead.top + lead.bottom) / 2;
				expect(startsNewLine(lead, { top: centre - reach, bottom: centre + reach })).toBe(false);
			})
		);
	});
});

describe('naturalSpan', () => {
	it('is 0 for nothing', () => {
		expect(naturalSpan([])).toBe(0);
	});
	it('runs from the leftmost edge to the rightmost', () => {
		expect(
			naturalSpan([
				{ left: 200, right: 370 },
				{ left: 382, right: 422 },
				{ left: 434, right: 620 }
			])
		).toBe(420);
	});
	it('measures a right-to-left line the same way', () => {
		expect(
			naturalSpan([
				{ left: 434, right: 620 },
				{ left: 382, right: 422 },
				{ left: 200, right: 370 }
			])
		).toBe(420);
	});
});

describe('naturalSpan, by property', () => {
	it('is the same in any order, and never less than any one box', () => {
		const span = fc
			.tuple(fc.integer({ min: 0, max: 2000 }), fc.integer({ min: 0, max: 400 }))
			.map(([left, w]) => ({ left, right: left + w }));
		fc.assert(
			fc.property(fc.array(span, { minLength: 1, maxLength: 6 }), (boxes) => {
				const n = naturalSpan(boxes);
				expect(naturalSpan([...boxes].reverse())).toBe(n);
				for (const b of boxes) expect(n).toBeGreaterThanOrEqual(b.right - b.left);
			})
		);
	});
});

describe('statusWraps', () => {
	const fit = { available: 1000, title: 600, sep: 6, status: 200, gap: 12 };
	it('is false while title, separator and status fit, with a gap either side of the separator', () => {
		expect(statusWraps(fit)).toBe(false);
		expect(statusWraps({ ...fit, available: 600 + 12 + 6 + 12 + 200 })).toBe(false);
	});
	it('is true once they do not', () => {
		expect(statusWraps({ ...fit, available: 600 + 12 + 6 + 12 + 199 })).toBe(true);
	});
});

describe('statusWraps, by property', () => {
	const w = fc.integer({ min: 0, max: 2000 });
	it('never wraps a status that a narrower header fits', () => {
		fc.assert(
			fc.property(
				w,
				w,
				w,
				w,
				fc.integer({ min: 0, max: 400 }),
				(title, sep, status, avail, more) => {
					const f = { title, sep, status, gap: 12, available: avail };
					if (!statusWraps(f)) expect(statusWraps({ ...f, available: avail + more })).toBe(false);
				}
			)
		);
	});

	it('depends only on widths: the same widths always answer the same', () => {
		fc.assert(
			fc.property(w, w, w, w, (title, sep, status, available) => {
				const f = { title, sep, status, gap: 12, available };
				expect(statusWraps(f)).toBe(title + 12 + sep + 12 + status > available);
			})
		);
	});
});

/** An element whose boxes the test sets: no layout engine runs here. */
interface Box {
	left: number;
	right: number;
	top: number;
	bottom: number;
}
type FakeEl = Element & { box: Box; kids: Box[] };
function el(b: Box, children: Box[] = []): FakeEl {
	const node = {
		box: b,
		kids: children,
		getBoundingClientRect: () => ({ ...node.box, width: node.box.right - node.box.left }),
		get children() {
			return node.kids.map((k) => el(k));
		},
		get clientWidth() {
			return node.box.right - node.box.left;
		}
	};
	return node as unknown as FakeEl;
}

class FakeObserver {
	static last: FakeObserver | null = null;
	observed: Element[] = [];
	disconnected = false;
	constructor(public cb: () => void) {
		FakeObserver.last = this;
	}
	observe(e: Element) {
		this.observed.push(e);
	}
	disconnect() {
		this.disconnected = true;
	}
}

const deps: HeaderDeps = {
	Observer: FakeObserver,
	styleOf: () => ({ padStart: 64, padEnd: 64, gap: 12 })
};

/** A header whose content starts at x=64; the title's content is 400
 *  wide, the separator 6, the status 200: on one line they need
 *  400 + 12 + 6 + 12 + 200 = 630. */
function header(width: number) {
	return {
		header: el({ left: 0, right: width, top: 0, bottom: 60 }),
		title: el({ left: 64, right: 64 + 400, top: 0, bottom: 16 }, [
			{ left: 64, right: 200, top: 0, bottom: 16 },
			{ left: 212, right: 252, top: 7, bottom: 8 },
			{ left: 264, right: 464, top: 0, bottom: 16 }
		]),
		sep: el({ left: 476, right: 482, top: 0, bottom: 16 }),
		status: el({ left: 494, right: 694, top: 0, bottom: 16 }),
		actions: el({ left: 706, right: 806, top: -8, bottom: 24 })
	};
}

describe('watchHeaderLines', () => {
	it('reports now, observes the header and its parts, and stops on teardown', () => {
		const parts = header(64 + 630 + 64);
		const seen: { metaWraps: boolean; actionsWrap: boolean }[] = [];
		const stop = watchHeaderLines(parts, (l) => seen.push(l), deps);
		expect(seen).toEqual([{ metaWraps: false, actionsWrap: false }]);
		const ro = FakeObserver.last!;
		for (const p of [parts.header, parts.title, parts.status, parts.actions]) {
			expect(ro.observed).toContain(p);
		}
		stop();
		expect(ro.disconnected).toBe(true);
	});

	it('wraps the status when the header narrows, and brings it back when it widens', () => {
		const parts = header(64 + 630 + 64);
		const seen: boolean[] = [];
		watchHeaderLines(parts, (l) => seen.push(l.metaWraps), deps);
		const ro = FakeObserver.last!;

		parts.header.box = { ...parts.header.box, right: 64 + 629 + 64 };
		ro.cb();
		expect(seen.at(-1)).toBe(true);

		// Strip answers the wrap: the separator is hidden (no box) and the
		// title takes the whole line. That layout must not hold the wrap.
		parts.sep.box = { left: 0, right: 0, top: 0, bottom: 0 };
		parts.title.box = { ...parts.title.box, right: 64 + 629 };
		ro.cb();
		expect(seen.at(-1)).toBe(true);

		parts.header.box = { ...parts.header.box, right: 1920 };
		parts.title.box = { ...parts.title.box, right: 1920 - 64 };
		ro.cb();
		expect(seen.at(-1)).toBe(false);
	});

	it('counts the hidden separator at the width it had when shown', () => {
		// Hidden, it would be 0 wide, and 630 − 6 would fit: the status
		// would come back beside the title, the separator return, and the
		// status wrap again.
		const parts = header(64 + 630 + 64);
		const seen: boolean[] = [];
		watchHeaderLines(parts, (l) => seen.push(l.metaWraps), deps);
		const ro = FakeObserver.last!;
		parts.header.box = { ...parts.header.box, right: 64 + 626 + 64 };
		ro.cb();
		parts.sep.box = { left: 0, right: 0, top: 0, bottom: 0 };
		ro.cb();
		expect(seen.slice(1)).toEqual([true, true]);
	});

	it('reports actions that sit below the title’s line', () => {
		const parts = header(1920);
		const seen: boolean[] = [];
		watchHeaderLines(parts, (l) => seen.push(l.actionsWrap), deps);
		parts.actions.box = { ...parts.actions.box, top: 32, bottom: 64 };
		FakeObserver.last!.cb();
		expect(seen).toEqual([false, true]);
	});

	it('works without a status or actions', () => {
		const parts = { ...header(1920), sep: null, status: null, actions: null };
		const seen: { metaWraps: boolean; actionsWrap: boolean }[] = [];
		watchHeaderLines(parts, (l) => seen.push(l), deps);
		expect(seen).toEqual([{ metaWraps: false, actionsWrap: false }]);
	});
});
