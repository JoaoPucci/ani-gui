// The Sub or Dub a correction carries to the show picked.

import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { rememberAudio, storedAudio, type AudioChoiceStorage } from '$lib/play/audio-choice';
import { carryAudio } from './carry-audio';

function memory(refuse = false): AudioChoiceStorage {
	const store = new Map<string, string>();
	return {
		getItem: (k) => store.get(k) ?? null,
		setItem: (k, v) => {
			if (refuse) throw new Error('quota');
			store.set(k, v);
		}
	};
}
const mode = fc.constantFrom<'sub' | 'dub' | null>('sub', 'dub', null);

describe('carryAudio', () => {
	it('gives the pick the audio watched in only when it has none of its own', () => {
		fc.assert(
			fc.property(
				mode,
				fc.option(fc.constantFrom<'sub' | 'dub'>('sub', 'dub'), { nil: undefined }),
				(watched, own) => {
					const storage = memory();
					if (own) rememberAudio('pick', own, storage);
					carryAudio({ audioInForce: () => watched }, 'shown', 'pick', storage);
					expect(storedAudio('pick', storage)).toBe(own ?? watched ?? undefined);
				}
			)
		);
	});

	it('asks for the audio of the show shown', () => {
		const asked: string[] = [];
		carryAudio({ audioInForce: (s) => (asked.push(s), 'dub') }, 'shown', 'pick', memory());
		expect(asked).toEqual(['shown']);
	});

	it('claims nothing, and throws nothing, when the storage refuses', () => {
		const storage = memory(true);
		expect(() => carryAudio({ audioInForce: () => 'dub' }, 'shown', 'pick', storage)).not.toThrow();
		expect(storedAudio('pick', storage)).toBeUndefined();
	});

	it('does nothing for a surface that knows no audio', () => {
		const storage = memory();
		carryAudio({}, 'shown', 'pick', storage);
		expect(storedAudio('pick', storage)).toBeUndefined();
	});
});
