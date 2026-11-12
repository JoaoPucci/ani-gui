import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { closedDrill, drillArrow, drillStep, type DrillEvent, type DrillMenu } from './drill-menu';

type L = 'speed' | 'quality';
const closed = closedDrill<L>();
const main: DrillMenu<L> = { open: true, list: null, width: null };
const inQuality: DrillMenu<L> = { open: true, list: 'quality', width: 231 };

describe('drillStep', () => {
	it('opens onto the main list, focus on its first item, and closes again', () => {
		expect(drillStep(closed, { type: 'toggle' })).toEqual({ menu: main, focus: { kind: 'first' } });
		expect(drillStep(inQuality, { type: 'toggle' })).toEqual({ menu: closed, focus: null });
	});

	it('closes leaving focus alone, or dismisses onto the menu button', () => {
		expect(drillStep(inQuality, { type: 'close' })).toEqual({ menu: closed, focus: null });
		expect(drillStep(inQuality, { type: 'dismiss' })).toEqual({
			menu: closed,
			focus: { kind: 'anchor' }
		});
	});

	it('opens a list from its row, keeping the width it is given, focus on the back row', () => {
		expect(
			drillStep(main, { type: 'openList', list: 'quality', choosable: true, width: 231 })
		).toEqual({ menu: inQuality, focus: { kind: 'first' } });
	});

	it('opens no list with nothing to choose, from a closed menu, or from another list', () => {
		const open = (menu: DrillMenu<L>, choosable: boolean) =>
			drillStep(menu, { type: 'openList', list: 'speed', choosable, width: null });
		expect(open(main, false)).toEqual({ menu: main, focus: null });
		expect(open(closed, true)).toEqual({ menu: closed, focus: null });
		expect(open(inQuality, true)).toEqual({ menu: inQuality, focus: null });
	});

	it('goes back to the main list onto the row that opened the list', () => {
		expect(drillStep(inQuality, { type: 'back' })).toEqual({
			menu: main,
			focus: { kind: 'opener', list: 'quality' }
		});
		expect(drillStep(main, { type: 'back' })).toEqual({ menu: main, focus: null });
	});

	it('Escape goes back from a list and dismisses from the main list', () => {
		expect(drillStep(inQuality, { type: 'escape' })).toEqual({
			menu: main,
			focus: { kind: 'opener', list: 'quality' }
		});
		expect(drillStep(main, { type: 'escape' })).toEqual({
			menu: closed,
			focus: { kind: 'anchor' }
		});
	});

	it('closes a list whose model has nothing left to choose, focus in the menu or on its button', () => {
		const models = (choosable: (l: L) => boolean) => ({ type: 'models' as const, choosable });
		expect(
			drillStep(
				inQuality,
				models((l) => l !== 'quality')
			)
		).toEqual({
			menu: main,
			focus: { kind: 'first-or-anchor' }
		});
		expect(
			drillStep(
				inQuality,
				models(() => true)
			)
		).toEqual({ menu: inQuality, focus: null });
		expect(
			drillStep(
				main,
				models(() => false)
			)
		).toEqual({ menu: main, focus: null });
		expect(
			drillStep(
				closed,
				models(() => false)
			)
		).toEqual({ menu: closed, focus: null });
	});
});

describe('drillArrow', () => {
	const at = (over: Partial<Parameters<typeof drillArrow<L>>[2]> = {}) => ({
		rtl: false,
		opener: null as L | null,
		choosable: true,
		width: 190,
		...over
	});

	it('opens the list from its row with Right, and goes back with Left', () => {
		expect(drillArrow(main, 'ArrowRight', at({ opener: 'speed' }))).toEqual({
			menu: { open: true, list: 'speed', width: 190 },
			focus: { kind: 'first' }
		});
		expect(drillArrow(inQuality, 'ArrowLeft', at())).toEqual({
			menu: main,
			focus: { kind: 'opener', list: 'quality' }
		});
	});

	it('mirrors both where the page reads right to left', () => {
		expect(drillArrow(main, 'ArrowLeft', at({ rtl: true, opener: 'speed' }))?.menu.list).toBe(
			'speed'
		);
		expect(drillArrow(inQuality, 'ArrowRight', at({ rtl: true }))?.menu).toEqual(main);
	});

	it('keeps any other horizontal arrow in the menu, unchanged', () => {
		expect(drillArrow(main, 'ArrowRight', at())).toEqual({ menu: main, focus: null });
		expect(drillArrow(main, 'ArrowLeft', at({ opener: 'speed' }))).toEqual({
			menu: main,
			focus: null
		});
		expect(drillArrow(main, 'ArrowRight', at({ opener: 'quality', choosable: false }))).toEqual({
			menu: main,
			focus: null
		});
	});

	it('leaves every other key to the menu’s own stepping or the page', () => {
		fc.assert(
			fc.property(
				fc.string().filter((k) => k !== 'ArrowLeft' && k !== 'ArrowRight'),
				(key) => {
					expect(drillArrow(inQuality, key, at())).toBeNull();
				}
			)
		);
	});
});

const list = fc.constantFrom<L>('speed', 'quality');
const event: fc.Arbitrary<DrillEvent<L>> = fc.oneof(
	fc.constant({ type: 'toggle' as const }),
	fc.constant({ type: 'close' as const }),
	fc.constant({ type: 'dismiss' as const }),
	fc.constant({ type: 'back' as const }),
	fc.constant({ type: 'escape' as const }),
	fc.record({
		type: fc.constant('openList' as const),
		list,
		choosable: fc.boolean(),
		width: fc.option(fc.nat(600), { nil: null })
	}),
	fc.record({ type: fc.constant('models' as const), keep: fc.boolean() }).map(({ keep }) => ({
		type: 'models' as const,
		choosable: () => keep
	}))
);

describe('drillStep properties', () => {
	it('never shows a list or keeps a width while closed, nor a width on the main list', () => {
		fc.assert(
			fc.property(fc.array(event, { maxLength: 30 }), (events) => {
				let menu = closed;
				for (const e of events) {
					menu = drillStep(menu, e).menu;
					if (!menu.open) expect(menu.list).toBeNull();
					if (menu.list === null) expect(menu.width).toBeNull();
				}
			})
		);
	});

	it('Escape from a list never closes the menu, and from the main list always does', () => {
		fc.assert(
			fc.property(fc.array(event, { maxLength: 30 }), (events) => {
				const menu = events.reduce((m, e) => drillStep(m, e).menu, closed);
				const after = drillStep(menu, { type: 'escape' });
				if (menu.list !== null) {
					expect(after.menu.open).toBe(true);
					expect(after.focus).toEqual({ kind: 'opener', list: menu.list });
				} else {
					expect(after.menu.open).toBe(false);
				}
			})
		);
	});

	it('opening a list and going back lands on its row with the main list as it was', () => {
		fc.assert(
			fc.property(list, fc.option(fc.nat(600), { nil: null }), (l, width) => {
				const opened = drillStep(main, { type: 'openList', list: l, choosable: true, width });
				const back = drillStep(opened.menu, { type: 'back' });
				expect(back).toEqual({ menu: main, focus: { kind: 'opener', list: l } });
			})
		);
	});

	it('a focus target inside the menu is only named while the menu is open', () => {
		fc.assert(
			fc.property(fc.array(event, { maxLength: 30 }), (events) => {
				let menu = closed;
				for (const e of events) {
					const step = drillStep(menu, e);
					if (step.focus?.kind === 'first' || step.focus?.kind === 'opener') {
						expect(step.menu.open).toBe(true);
					}
					menu = step.menu;
				}
			})
		);
	});

	it('a step that changes nothing hands back the same menu, so an effect that runs it settles', () => {
		fc.assert(
			fc.property(fc.array(event, { maxLength: 30 }), event, (events, e) => {
				const menu = events.reduce((m, ev) => drillStep(m, ev).menu, closed);
				const after = drillStep(menu, e).menu;
				const same =
					after.open === menu.open && after.list === menu.list && after.width === menu.width;
				if (same) expect(after).toBe(menu);
			})
		);
	});
});
