import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { audioFor, rememberAudio, storedAudio, type AudioChoiceStorage } from './audio-choice';

function memory(): AudioChoiceStorage & { raw: Map<string, string> } {
	const raw = new Map<string, string>();
	return {
		raw,
		getItem: (k) => raw.get(k) ?? null,
		setItem: (k, v) => void raw.set(k, v)
	};
}

describe('per-show audio', () => {
	it('a show never chosen on has no audio of its own, and plays the Settings default', () => {
		const s = memory();
		expect(storedAudio('42', s)).toBeUndefined();
		expect(audioFor('42', 'sub', s)).toBe('sub');
		expect(audioFor('42', 'dub', s)).toBe('dub');
	});

	it("a show's choice is its own, whatever the default", () => {
		const s = memory();
		expect(rememberAudio('42', 'dub', s)).toBe(true);
		expect(storedAudio('42', s)).toBe('dub');
		expect(audioFor('42', 'sub', s)).toBe('dub');
		expect(audioFor('43', 'sub', s)).toBe('sub');
	});

	it('a later choice replaces the earlier one', () => {
		const s = memory();
		rememberAudio('42', 'dub', s);
		rememberAudio('42', 'sub', s);
		expect(audioFor('42', 'dub', s)).toBe('sub');
	});

	it('a page with no Kitsu id, or no storage, remembers nothing and plays the default', () => {
		const s = memory();
		expect(rememberAudio('', 'dub', s)).toBe(false);
		expect(rememberAudio('42', 'dub', null)).toBe(false);
		expect(audioFor('', 'sub', s)).toBe('sub');
		expect(audioFor(null, 'dub', s)).toBe('dub');
		expect(audioFor('42', 'sub', null)).toBe('sub');
	});

	it('a corrupt or refusing storage reads as no choice', () => {
		const s = memory();
		s.raw.set('ani-gui.audio-choices', '{not json');
		expect(storedAudio('42', s)).toBeUndefined();
		s.raw.set('ani-gui.audio-choices', JSON.stringify([['42', 'raw'], ['43', 'dub'], 7]));
		expect(storedAudio('42', s)).toBeUndefined();
		expect(storedAudio('43', s)).toBe('dub');
		const refusing: AudioChoiceStorage = {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('denied');
			}
		};
		expect(storedAudio('42', refusing)).toBeUndefined();
		expect(rememberAudio('42', 'dub', refusing)).toBe(false);
	});

	it('every show keeps its own last choice, and a show never chosen plays the default', () => {
		fc.assert(
			fc.property(
				fc.array(fc.tuple(fc.constantFrom('1', '2', '3', '4'), fc.constantFrom('sub', 'dub')), {
					maxLength: 20
				}),
				fc.constantFrom('sub', 'dub'),
				(picks, fallback) => {
					const s = memory();
					const last = new Map<string, 'sub' | 'dub'>();
					for (const [id, mode] of picks) {
						rememberAudio(id, mode as 'sub' | 'dub', s);
						last.set(id, mode as 'sub' | 'dub');
					}
					for (const id of ['1', '2', '3', '4', '5']) {
						expect(audioFor(id, fallback as 'sub' | 'dub', s)).toBe(last.get(id) ?? fallback);
					}
				}
			)
		);
	});
});
