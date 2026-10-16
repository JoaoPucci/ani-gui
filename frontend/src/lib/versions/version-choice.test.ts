import { describe, expect, it } from 'vitest';
import type { VersionInfo } from '$lib/api';
import {
	MAX_CHOICES,
	applyChoice,
	defaultStorage,
	rememberChoice,
	selectedVersion,
	storedChoice,
	versionsToOffer,
	type ChoiceStorage
} from './version-choice';

const KEY = 'ani-gui.version-choices';

/** An in-memory storage, with a peek at what was written. */
function memory(initial: Record<string, string> = {}) {
	const data = new Map(Object.entries(initial));
	const storage: ChoiceStorage = {
		getItem: (k) => data.get(k) ?? null,
		setItem: (k, v) => void data.set(k, v)
	};
	return { storage, raw: () => data.get(KEY) };
}

/** A storage whose every access throws, as a blocked one does. */
const throwing: ChoiceStorage = {
	getItem: () => {
		throw new Error('blocked');
	},
	setItem: () => {
		throw new Error('blocked');
	}
};

const uncensored: VersionInfo = { kind: 'uncensored', show_key: 'hianime:show-uncensored-1' };
const standard: VersionInfo = { kind: 'standard', show_key: 'hianime:show-2' };

describe('the remembered version of a show', () => {
	it('is nothing until one is chosen', () => {
		expect(storedChoice('42', memory().storage)).toBeUndefined();
	});

	it('is the one chosen, per Kitsu id', () => {
		const { storage } = memory();
		expect(rememberChoice('42', 'standard', storage)).toBe(true);
		expect(rememberChoice('7', 'uncensored', storage)).toBe(true);
		expect(storedChoice('42', storage)).toBe('standard');
		expect(storedChoice('7', storage)).toBe('uncensored');
		expect(storedChoice('8', storage)).toBeUndefined();
	});

	it('is the latest choice when a show is chosen for again', () => {
		const { storage } = memory();
		rememberChoice('42', 'standard', storage);
		rememberChoice('42', 'uncensored', storage);
		expect(storedChoice('42', storage)).toBe('uncensored');
	});

	it('survives a reload: it lives in the storage, not in the module', () => {
		const first = memory();
		rememberChoice('42', 'standard', first.storage);
		const reloaded = memory({ [KEY]: first.raw()! });
		expect(storedChoice('42', reloaded.storage)).toBe('standard');
	});

	it('keeps one entry per show, the newest last, and at most MAX_CHOICES', () => {
		const { storage, raw } = memory();
		for (let i = 0; i < MAX_CHOICES + 5; i += 1) rememberChoice(String(i), 'standard', storage);
		rememberChoice('10', 'uncensored', storage);
		const kept = JSON.parse(raw()!) as [string, string][];
		expect(kept).toHaveLength(MAX_CHOICES);
		expect(kept.at(-1)).toEqual(['10', 'uncensored']);
		expect(kept.filter(([id]) => id === '10')).toHaveLength(1);
		// The oldest choices are the ones let go.
		expect(storedChoice('0', storage)).toBeUndefined();
		expect(storedChoice(String(MAX_CHOICES + 4), storage)).toBe('standard');
	});

	it('is not recorded for a page with no Kitsu id', () => {
		const { storage, raw } = memory();
		expect(rememberChoice('', 'standard', storage)).toBe(false);
		expect(raw()).toBeUndefined();
		expect(storedChoice('', storage)).toBeUndefined();
	});

	it('reads as nothing from corrupt or foreign contents', () => {
		expect(storedChoice('42', memory({ [KEY]: '{not json' }).storage)).toBeUndefined();
		expect(storedChoice('42', memory({ [KEY]: '{"42":"standard"}' }).storage)).toBeUndefined();
		const mixed = memory({
			[KEY]: JSON.stringify([
				['42', 'director'],
				['7', 'standard'],
				[42, 'standard'],
				'junk',
				['9']
			])
		});
		expect(storedChoice('42', mixed.storage)).toBeUndefined();
		expect(storedChoice('7', mixed.storage)).toBe('standard');
		expect(storedChoice('9', mixed.storage)).toBeUndefined();
	});

	it('drops corrupt entries on the next write rather than keeping them', () => {
		const { storage, raw } = memory({ [KEY]: JSON.stringify([['42', 'director'], 'junk']) });
		rememberChoice('7', 'standard', storage);
		expect(JSON.parse(raw()!)).toEqual([['7', 'standard']]);
	});

	it('reads as nothing, and writes report false, when the storage refuses', () => {
		expect(storedChoice('42', throwing)).toBeUndefined();
		expect(rememberChoice('42', 'standard', throwing)).toBe(false);
		expect(storedChoice('42', null)).toBeUndefined();
		expect(rememberChoice('42', 'standard', null)).toBe(false);
	});

	it('defaults to the renderer local storage', () => {
		const g = globalThis as { localStorage?: unknown };
		const saved = Object.getOwnPropertyDescriptor(g, 'localStorage');
		const { storage } = memory();
		Object.defineProperty(g, 'localStorage', { configurable: true, value: storage });
		try {
			expect(defaultStorage()).toBe(storage);
			rememberChoice('42', 'standard');
			expect(storedChoice('42')).toBe('standard');
		} finally {
			if (saved) Object.defineProperty(g, 'localStorage', saved);
			else delete g.localStorage;
		}
	});

	it('defaults to no storage when there is none, or reading it throws', () => {
		const g = globalThis as { localStorage?: unknown };
		const saved = Object.getOwnPropertyDescriptor(g, 'localStorage');
		try {
			Object.defineProperty(g, 'localStorage', { configurable: true, value: undefined });
			expect(defaultStorage()).toBeNull();
			Object.defineProperty(g, 'localStorage', {
				configurable: true,
				get() {
					throw new Error('SecurityError');
				}
			});
			expect(defaultStorage()).toBeNull();
		} finally {
			if (saved) Object.defineProperty(g, 'localStorage', saved);
			else delete g.localStorage;
		}
	});
});

describe('the versions a show offers', () => {
	it('are none when the show has one version, or the answer names none', () => {
		expect(versionsToOffer(undefined)).toEqual([]);
		expect(versionsToOffer(null)).toEqual([]);
		expect(versionsToOffer([])).toEqual([]);
		expect(versionsToOffer([uncensored])).toEqual([]);
	});

	it("are every version, in the provider's order, when it has several", () => {
		expect(versionsToOffer([standard, uncensored])).toEqual([standard, uncensored]);
	});
});

describe('the version selected for a show', () => {
	it('is none when nothing is offered', () => {
		expect(selectedVersion([], undefined)).toBeUndefined();
		expect(selectedVersion([], 'standard')).toBeUndefined();
	});

	it('is the remembered one when the show carries it', () => {
		expect(selectedVersion([uncensored, standard], 'standard')).toBe('standard');
		expect(selectedVersion([standard, uncensored], 'uncensored')).toBe('uncensored');
	});

	it('is the uncensored one when nothing is remembered, wherever it is listed', () => {
		expect(selectedVersion([standard, uncensored], undefined)).toBe('uncensored');
	});

	it('is the first listed when the show has neither the remembered kind nor an uncensored one', () => {
		const twoStandards: VersionInfo[] = [standard, { kind: 'standard', show_key: 'hianime:x' }];
		expect(selectedVersion(twoStandards, 'uncensored')).toBe('standard');
	});
});

describe('applying a choice', () => {
	it('remembers a choice of the plain copy and names it from then on', () => {
		const { storage } = memory();
		expect(applyChoice('42', 'standard', storage)).toBe('standard');
		expect(storedChoice('42', storage)).toBe('standard');
	});

	it('forgets the choice when the default is chosen, so requests name none again', () => {
		// The backend plays the uncensored copy for a request naming no
		// version; naming it explicitly would warm and cache the same
		// listing a second time under another key.
		const { storage } = memory();
		applyChoice('42', 'standard', storage);
		applyChoice('7', 'standard', storage);
		expect(applyChoice('42', 'uncensored', storage)).toBeUndefined();
		expect(storedChoice('42', storage)).toBeUndefined();
		expect(storedChoice('7', storage)).toBe('standard');
	});

	it('names what the storage holds when it refuses the write', () => {
		// The play page reads the choice from storage; a page playing a
		// choice the storage never took would pair its play with
		// another page's eviction and stamp naming none.
		const refusing: ChoiceStorage = {
			getItem: () => JSON.stringify([['42', 'standard']]),
			setItem: () => {
				throw new Error('quota');
			}
		};
		expect(applyChoice('42', 'uncensored', refusing)).toBe('standard');
		expect(applyChoice('9', 'standard', throwing)).toBeUndefined();
		expect(applyChoice('9', 'standard', null)).toBeUndefined();
	});
});
