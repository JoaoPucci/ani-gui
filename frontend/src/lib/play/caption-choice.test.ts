// @vitest-environment happy-dom
import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import {
	captionChoiceOf,
	defaultStorage,
	MAX_CAPTION_CHOICES,
	rememberCaption,
	storedCaption,
	type CaptionChoiceStorage
} from './caption-choice';

function memory(): CaptionChoiceStorage & { data: Map<string, string> } {
	const data = new Map<string, string>();
	return {
		data,
		getItem: (k) => data.get(k) ?? null,
		setItem: (k, v) => void data.set(k, v)
	};
}

describe('the remembered captions choice', () => {
	it('is kept per show', () => {
		const s = memory();
		expect(rememberCaption('42', 'es-419', s)).toBe(true);
		expect(rememberCaption('7', 'off', s)).toBe(true);
		expect(storedCaption('42', s)).toBe('es-419');
		expect(storedCaption('7', s)).toBe('off');
		expect(storedCaption('99', s)).toBeUndefined();
	});

	it('is replaced by the next choice on the same show', () => {
		const s = memory();
		rememberCaption('42', 'es-419', s);
		rememberCaption('42', 'off', s);
		expect(storedCaption('42', s)).toBe('off');
	});

	it('keeps the most recent shows when there are too many', () => {
		const s = memory();
		for (let i = 0; i <= MAX_CAPTION_CHOICES; i++) rememberCaption(String(i), 'en', s);
		expect(storedCaption('0', s)).toBeUndefined();
		expect(storedCaption(String(MAX_CAPTION_CHOICES), s)).toBe('en');
	});

	it('reads a corrupt or refused storage as no choice, and remembers nothing without a show', () => {
		const corrupt = memory();
		corrupt.data.set('ani-gui.caption-choices', '{not json');
		expect(storedCaption('42', corrupt)).toBeUndefined();
		const refusing: CaptionChoiceStorage = {
			getItem: () => null,
			setItem: () => {
				throw new Error('quota');
			}
		};
		expect(rememberCaption('42', 'en', refusing)).toBe(false);
		expect(rememberCaption('', 'en', memory())).toBe(false);
		expect(rememberCaption('42', 'en', null)).toBe(false);
	});

	it('round-trips any show and choice, leaving the other shows as they were', () => {
		fc.assert(
			fc.property(
				fc.array(
					fc.tuple(
						fc.stringMatching(/^[0-9]{1,6}$/),
						fc.constantFrom('off', 'en', 'pt-BR', 'es-419', 'zh-Hant')
					),
					{
						maxLength: 30
					}
				),
				(writes) => {
					const s = memory();
					const last = new Map<string, string>();
					for (const [id, choice] of writes) {
						rememberCaption(id, choice, s);
						last.set(id, choice);
					}
					for (const [id, choice] of last) expect(storedCaption(id, s)).toBe(choice);
				}
			)
		);
	});
});

describe('captionChoiceOf', () => {
	const list = [
		{ kind: 'metadata', language: '' },
		{ kind: 'subtitles', language: 'pt-BR' },
		{ kind: 'captions', language: 'es-419' },
		{ kind: 'subtitles', language: 'und' },
		{ kind: 'subtitles', language: '' }
	];

	it('remembers off, and a picked track by its language tag', () => {
		expect(captionChoiceOf(list, 'off')).toBe('off');
		expect(captionChoiceOf(list, 1)).toBe('pt-BR');
		expect(captionChoiceOf(list, 2)).toBe('es-419');
	});

	it('remembers nothing for a track of no known language, a non-caption track, or no track', () => {
		expect(captionChoiceOf(list, 3)).toBeUndefined();
		expect(captionChoiceOf(list, 4)).toBeUndefined();
		expect(captionChoiceOf(list, 0)).toBeUndefined();
		expect(captionChoiceOf(list, 9)).toBeUndefined();
	});
});

describe('the default store', () => {
	it("is the renderer's local storage, and none where reading it throws", () => {
		expect(defaultStorage()).toBe(globalThis.localStorage);
		const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
		Object.defineProperty(globalThis, 'localStorage', {
			configurable: true,
			get() {
				throw new Error('denied');
			}
		});
		try {
			expect(defaultStorage()).toBeNull();
			expect(storedCaption('42')).toBeUndefined();
		} finally {
			if (descriptor) Object.defineProperty(globalThis, 'localStorage', descriptor);
		}
	});
});
