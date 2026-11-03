import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { menuHeight, menuKeyStep, revealScroll } from './caption-menu-fit';

describe('menuHeight — the captions menu fits inside the player', () => {
	it('is the room between the top of the player and the button, less the gaps', () => {
		// Player from y=84, button at y=420, 8 px under the menu and 8 above it.
		expect(menuHeight({ frameTop: 84, anchorTop: 420, gap: 8, margin: 8 })).toBe(320);
	});

	it('rounds down to whole pixels', () => {
		expect(menuHeight({ frameTop: 0.5, anchorTop: 100.9, gap: 0, margin: 0 })).toBe(100);
	});

	it('is never negative, however small the player', () => {
		expect(menuHeight({ frameTop: 100, anchorTop: 90, gap: 8, margin: 8 })).toBe(0);
	});

	it('stops at the top bar when the page has scrolled the player up under it', () => {
		// Player scrolled to y=-200; the top bar ends at y=84.
		expect(menuHeight({ frameTop: -200, ceiling: 84, anchorTop: 420, gap: 8, margin: 8 })).toBe(
			320
		);
		expect(menuHeight({ frameTop: 100, ceiling: 84, anchorTop: 420, gap: 8, margin: 8 })).toBe(304);
	});

	it('keeps a floor, so a player too short for the menu still caps it', () => {
		expect(menuHeight({ frameTop: 100, anchorTop: 90, gap: 8, margin: 8, floor: 96 })).toBe(96);
		expect(menuHeight({ frameTop: 84, anchorTop: 420, gap: 8, margin: 8, floor: 96 })).toBe(320);
	});

	it('never reaches above the player or the top bar, unless only the floor fits', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -2000, max: 2000 }),
				fc.integer({ min: -2000, max: 2000 }),
				fc.integer({ min: -2000, max: 2000 }),
				fc.nat(50),
				fc.nat(50),
				fc.nat(200),
				(frameTop, ceiling, anchorTop, gap, margin, floor) => {
					const h = menuHeight({ frameTop, ceiling, anchorTop, gap, margin, floor });
					expect(h).toBeGreaterThanOrEqual(floor);
					expect(Number.isInteger(h)).toBe(true);
					if (h > floor) {
						expect(anchorTop - gap - h).toBeGreaterThanOrEqual(
							Math.max(frameTop, ceiling) + margin
						);
					}
				}
			)
		);
	});

	it("stops at half the player's height, however much room is above the button", () => {
		// A 1000 px player: 884 px of room above the button, 500 of it kept.
		expect(
			menuHeight({ frameTop: 100, frameHeight: 1000, anchorTop: 1000, gap: 8, margin: 8 })
		).toBe(500);
		// Less room than half the player: the room.
		expect(menuHeight({ frameTop: 84, frameHeight: 1000, anchorTop: 420, gap: 8, margin: 8 })).toBe(
			320
		);
		// Half of an odd height rounds down.
		expect(menuHeight({ frameTop: 0, frameHeight: 601, anchorTop: 600, gap: 0, margin: 0 })).toBe(
			300
		);
	});

	it('keeps the floor in a player too short for half of it to hold two entries', () => {
		expect(
			menuHeight({ frameTop: 0, frameHeight: 150, anchorTop: 140, gap: 8, margin: 8, floor: 96 })
		).toBe(96);
	});

	it('is never above half the player, nor the room, unless the floor applies; never below the floor', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -2000, max: 2000 }),
				fc.integer({ min: -2000, max: 2000 }),
				fc.nat(3000),
				fc.integer({ min: -2000, max: 2000 }),
				fc.nat(50),
				fc.nat(50),
				fc.nat(200),
				(frameTop, ceiling, frameHeight, anchorTop, gap, margin, floor) => {
					const h = menuHeight({
						frameTop,
						ceiling,
						frameHeight,
						anchorTop,
						gap,
						margin,
						floor
					});
					expect(h).toBeGreaterThanOrEqual(floor);
					expect(Number.isInteger(h)).toBe(true);
					if (h > floor) {
						expect(h).toBeLessThanOrEqual(frameHeight / 2);
						expect(anchorTop - gap - h).toBeGreaterThanOrEqual(
							Math.max(frameTop, ceiling) + margin
						);
					}
				}
			)
		);
	});

	it('never reaches above the player', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: -2000, max: 2000 }),
				fc.integer({ min: -2000, max: 2000 }),
				fc.nat(50),
				fc.nat(50),
				(frameTop, anchorTop, gap, margin) => {
					const h = menuHeight({ frameTop, anchorTop, gap, margin });
					expect(h).toBeGreaterThanOrEqual(0);
					expect(Number.isInteger(h)).toBe(true);
					if (h > 0) expect(anchorTop - gap - h).toBeGreaterThanOrEqual(frameTop + margin);
				}
			)
		);
	});
});

describe('revealScroll — the selected entry is in view when the menu opens', () => {
	it('keeps the scroll where the entry is already in view', () => {
		expect(revealScroll({ scrollTop: 0, viewHeight: 300, itemTop: 100, itemHeight: 36 })).toBe(0);
	});

	it('scrolls down just far enough for an entry below the view', () => {
		expect(revealScroll({ scrollTop: 0, viewHeight: 300, itemTop: 900, itemHeight: 36 })).toBe(636);
	});

	it('scrolls up to an entry above the view', () => {
		expect(revealScroll({ scrollTop: 500, viewHeight: 300, itemTop: 120, itemHeight: 36 })).toBe(
			120
		);
	});

	it('leaves the entry wholly in view whenever it fits', () => {
		fc.assert(
			fc.property(
				fc.nat(5000),
				fc.integer({ min: 1, max: 1000 }),
				fc.nat(5000),
				fc.integer({ min: 1, max: 100 }),
				(scrollTop, viewHeight, itemTop, itemHeight) => {
					fc.pre(itemHeight <= viewHeight);
					const top = revealScroll({ scrollTop, viewHeight, itemTop, itemHeight });
					expect(top).toBeLessThanOrEqual(itemTop);
					expect(top + viewHeight).toBeGreaterThanOrEqual(itemTop + itemHeight);
				}
			)
		);
	});
});

describe('menuKeyStep — arrow keys move through the entries', () => {
	it('moves down and up, wrapping at the ends', () => {
		expect(menuKeyStep('ArrowDown', 0, 5)).toBe(1);
		expect(menuKeyStep('ArrowDown', 4, 5)).toBe(0);
		expect(menuKeyStep('ArrowUp', 0, 5)).toBe(4);
		expect(menuKeyStep('ArrowUp', 3, 5)).toBe(2);
	});

	it('goes to the first and last with Home and End', () => {
		expect(menuKeyStep('Home', 3, 5)).toBe(0);
		expect(menuKeyStep('End', 1, 5)).toBe(4);
	});

	it('starts from the ends when no entry has focus', () => {
		expect(menuKeyStep('ArrowDown', -1, 5)).toBe(0);
		expect(menuKeyStep('ArrowUp', -1, 5)).toBe(4);
	});

	it('leaves every other key, and an empty menu, to the browser', () => {
		expect(menuKeyStep('Enter', 0, 5)).toBeNull();
		expect(menuKeyStep('ArrowLeft', 0, 5)).toBeNull();
		expect(menuKeyStep('ArrowDown', 0, 0)).toBeNull();
	});

	it('always lands on an entry', () => {
		fc.assert(
			fc.property(
				fc.constantFrom('ArrowDown', 'ArrowUp', 'Home', 'End'),
				fc.integer({ min: -1, max: 60 }),
				fc.integer({ min: 1, max: 60 }),
				(key, current, count) => {
					fc.pre(current < count);
					const next = menuKeyStep(key, current, count);
					expect(next).not.toBeNull();
					expect(next).toBeGreaterThanOrEqual(0);
					expect(next).toBeLessThan(count);
				}
			)
		);
	});
});
