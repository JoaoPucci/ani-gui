import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { startsNewLine, watchHeaderWrap, type LineBox } from './strip-header-wrap';

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
		// The title itself wrapped: its box spans both lines, and the
		// status sits beside the caption on the second.
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

/** An element whose box the test sets: no layout engine runs here. */
function el(b: LineBox) {
	const node = { box: b, getBoundingClientRect: () => node.box } as unknown as Element & {
		box: LineBox;
	};
	return node;
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

describe('watchHeaderWrap', () => {
	it('reports now, again when the header resizes, and stops on teardown', () => {
		const header = el({ top: 0, bottom: 60 });
		const lead = el(TITLE);
		const item = el({ top: 92, bottom: 124 });
		const seen: boolean[] = [];
		const stop = watchHeaderWrap(header, lead, item, (w) => seen.push(w), FakeObserver);
		expect(seen).toEqual([false]);
		const ro = FakeObserver.last!;
		expect(ro.observed).toContain(header);

		item.box = { top: 124, bottom: 156 };
		ro.cb();
		expect(seen).toEqual([false, true]);

		stop();
		expect(ro.disconnected).toBe(true);
	});
});
