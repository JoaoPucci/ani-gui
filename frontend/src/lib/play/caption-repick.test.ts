import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { storedCaption, type CaptionChoiceStorage } from './caption-choice';
import { rememberPickMoved } from './caption-repick';

/** An in-memory storage, refusing writes when `refuse` is set. */
function memory(refuse = false): CaptionChoiceStorage {
	const items = new Map<string, string>();
	return {
		getItem: (k) => items.get(k) ?? null,
		setItem: (k, v) => {
			if (refuse) throw new Error('quota');
			items.set(k, v);
		}
	};
}

describe('rememberPickMoved — a captions pick and the warms', () => {
	it('moves the choice on a first pick and on a pick of another language', () => {
		const storage = memory();
		expect(rememberPickMoved('42', 'es-419', storage)).toBe(true);
		expect(rememberPickMoved('42', 'vi', storage)).toBe(true);
		expect(storedCaption('42', storage)).toBe('vi');
	});

	it('moves nothing on a pick of the language already remembered, which it still writes', () => {
		const storage = memory();
		rememberPickMoved('42', 'vi', storage);
		rememberPickMoved('7', 'en', storage);
		expect(rememberPickMoved('42', 'vi', storage)).toBe(false);
		// Written again: the most recent entry is the show's.
		expect(JSON.parse(storage.getItem('ani-gui.caption-choices') ?? '[]').at(-1)).toEqual([
			'42',
			'vi'
		]);
	});

	it('moves Off like any other choice', () => {
		const storage = memory();
		rememberPickMoved('42', 'vi', storage);
		expect(rememberPickMoved('42', 'off', storage)).toBe(true);
		expect(rememberPickMoved('42', 'off', storage)).toBe(false);
	});

	it('moves nothing for a guessed match, which has no show id', () => {
		const storage = memory();
		expect(rememberPickMoved('', 'vi', storage)).toBe(false);
	});

	it('moves nothing when the storage refuses the write, or there is none', () => {
		expect(rememberPickMoved('42', 'vi', memory(true))).toBe(false);
		expect(rememberPickMoved('42', 'vi', null)).toBe(false);
	});

	it('moves the choice exactly when the pick differs from what was remembered', () => {
		fc.assert(
			fc.property(
				fc.array(
					fc.tuple(fc.constantFrom('1', '2', ''), fc.constantFrom('en', 'vi', 'es-419', 'off')),
					{ maxLength: 30 }
				),
				(picks) => {
					const storage = memory();
					for (const [show, choice] of picks) {
						const before = storedCaption(show, storage);
						const moved = rememberPickMoved(show, choice, storage);
						expect(moved).toBe(show !== '' && before !== choice);
						if (show !== '') expect(storedCaption(show, storage)).toBe(choice);
					}
				}
			)
		);
	});
});
