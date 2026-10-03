import { afterEach, describe, expect, it } from 'vitest';
import {
	FINISHED_REMAINING_S,
	MAX_POSITIONS,
	RESUME_MIN_S,
	clearPosition,
	readPosition,
	savePosition,
	type PositionStorage
} from './watch-position';

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

	it('forgets an episode left at its start', () => {
		const s = memory();
		savePosition('42', 3, 612.4, 1420, s);
		savePosition('42', 3, RESUME_MIN_S - 1, 1420, s);
		expect(readPosition('42', 3, s)).toBeNull();
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
