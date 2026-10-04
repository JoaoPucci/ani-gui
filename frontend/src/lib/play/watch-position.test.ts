import { afterEach, describe, expect, it } from 'vitest';
import {
	FINISHED_REMAINING_S,
	MAX_POSITIONS,
	RESUME_MIN_S,
	clearAllPositions,
	clearPosition,
	clearShowPositions,
	markStarted,
	readPosition,
	savePosition,
	type PositionStorage
} from './watch-position';
import { recoveryResume } from './resume-after-recovery';

function memory(): PositionStorage & { data: Map<string, string> } {
	const data = new Map<string, string>();
	return {
		data,
		getItem: (k) => data.get(k) ?? null,
		setItem: (k, v) => void data.set(k, v)
	};
}

describe('watch position', () => {
	it('reads back where an episode was left', () => {
		const s = memory();
		savePosition('42', 3, 612.4, 1420, s);
		expect(readPosition('42', 3, s)).toBe(612.4);
		expect(readPosition('42', 4, s)).toBeNull();
		expect(readPosition('7', 3, s)).toBeNull();
	});

	it('keeps an episode left at its start as started, to start over', () => {
		// Its watched mark is already written, so forgetting it would
		// make Continue skip the episode the viewer barely began.
		const s = memory();
		savePosition('42', 3, 612.4, 1420, s);
		savePosition('42', 3, RESUME_MIN_S - 1, 1420, s);
		expect(readPosition('42', 3, s)).toBe(0);
	});

	it('keeps a point in the first seconds of a short stream as started, not finished', () => {
		// A 60-second stream left at 5 s is within 90 s of its end, but
		// a point in the first seconds is a start over, never a finish.
		const s = memory();
		savePosition('42', 3, 5, 60, s);
		expect(readPosition('42', 3, s)).toBe(0);
	});

	it('marks an episode started without moving a point already kept', () => {
		const s = memory();
		markStarted('42', 3, s);
		expect(readPosition('42', 3, s)).toBe(0);
		savePosition('42', 4, 612.4, 1420, s);
		markStarted('42', 4, s);
		expect(readPosition('42', 4, s)).toBe(612.4);
	});

	it('forgets an episode left in its last minutes, as finished', () => {
		const s = memory();
		savePosition('42', 3, 612.4, 1420, s);
		savePosition('42', 3, 1420 - FINISHED_REMAINING_S + 1, 1420, s);
		expect(readPosition('42', 3, s)).toBeNull();
	});

	it('keeps a position whose length is not known yet', () => {
		const s = memory();
		savePosition('42', 3, 612.4, Number.NaN, s);
		expect(readPosition('42', 3, s)).toBe(612.4);
	});

	it('clears an episode', () => {
		const s = memory();
		savePosition('42', 3, 612.4, 1420, s);
		clearPosition('42', 3, s);
		expect(readPosition('42', 3, s)).toBeNull();
	});

	it("forgets every kept episode of one show, and only that show's", () => {
		const s = memory();
		savePosition('42', 3, 612.4, 1420, s);
		savePosition('42', 4, 100, 1420, s);
		savePosition('420', 3, 200, 1420, s);
		clearShowPositions('42', s);
		expect(readPosition('42', 3, s)).toBeNull();
		expect(readPosition('42', 4, s)).toBeNull();
		expect(readPosition('420', 3, s)).toBe(200);
	});

	it("forgetting a show's episodes forgets a recovery's pending point for it too", () => {
		// A recovery that began before the viewer left still holds where
		// the stream stood; forgotten positions include it.
		const s = memory();
		recoveryResume.capture('42', 3, 700);
		clearShowPositions('42', s);
		expect(recoveryResume.consume('42', 3)).toBeNull();
	});

	it("forgetting every episode forgets a recovery's pending point too", () => {
		const s = memory();
		recoveryResume.capture('42', 3, 700);
		clearAllPositions(s);
		expect(recoveryResume.consume('42', 3)).toBeNull();
	});

	it('forgets every kept episode', () => {
		const s = memory();
		savePosition('42', 3, 612.4, 1420, s);
		savePosition('7', 1, 100, 1420, s);
		clearAllPositions(s);
		expect(readPosition('42', 3, s)).toBeNull();
		expect(readPosition('7', 1, s)).toBeNull();
	});

	it('keeps only the most recent episodes', () => {
		const s = memory();
		for (let ep = 1; ep <= MAX_POSITIONS + 5; ep++) savePosition('42', ep, 100, 1420, s);
		expect(readPosition('42', 1, s)).toBeNull();
		expect(readPosition('42', MAX_POSITIONS + 5, s)).toBe(100);
		expect(readPosition('42', 6, s)).toBe(100);
	});

	it('a storage that refuses or holds garbage reads as nothing and never throws', () => {
		const refusing: PositionStorage = {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('denied');
			}
		};
		expect(() => savePosition('42', 3, 612.4, 1420, refusing)).not.toThrow();
		expect(readPosition('42', 3, refusing)).toBeNull();
		const garbage = memory();
		garbage.setItem('ani-gui.watch-positions', '{not json');
		expect(readPosition('42', 3, garbage)).toBeNull();
		savePosition('42', 3, 612.4, 1420, garbage);
		expect(readPosition('42', 3, garbage)).toBe(612.4);
	});

	describe('the default store', () => {
		const original = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
		afterEach(() => {
			if (original) Object.defineProperty(globalThis, 'localStorage', original);
			else Reflect.deleteProperty(globalThis, 'localStorage');
		});

		it("is the renderer's local storage", () => {
			const s = memory();
			Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: s });
			savePosition('42', 3, 612.4, 1420);
			expect(readPosition('42', 3)).toBe(612.4);
			expect(readPosition('42', 3, s)).toBe(612.4);
			clearPosition('42', 3);
			expect(readPosition('42', 3, s)).toBeNull();
		});

		it('a storage the renderer refuses to hand over reads as nothing', () => {
			Object.defineProperty(globalThis, 'localStorage', {
				configurable: true,
				get: () => {
					throw new Error('denied');
				}
			});
			expect(() => savePosition('42', 3, 612.4, 1420)).not.toThrow();
			expect(readPosition('42', 3)).toBeNull();
		});
	});
});
