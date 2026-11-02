/**
 * Watch-order mockup preferences, kept per machine in local storage
 * like the hero framing switch's ($lib/hero/prefs). The diagnostics
 * page owns `enabled` and `switcher`; the on-page switch sets the
 * rest. `order` stands in for the app setting the feature will add.
 * Signatures only: every read gives the defaults, and nothing is kept.
 */
import type { StorageLike } from '$lib/hero/prefs';

export const MOCK_OPTIONS = {
	order: ['linked', 'released'],
	strip: ['rail', 'trio'],
	placement: ['top', 'bottom'],
	titles: ['full', 'short'],
	upNext: ['poster', 'still'],
	franchise: ['list', 'shelf'],
	setting: ['segmented', 'radio'],
	handoff: ['next', 'episode']
} as const;

type Options = typeof MOCK_OPTIONS;
export type MockOptionKey = keyof Options;
export type MockChoices = { [K in MockOptionKey]: Options[K][number] };

export interface WatchOrderMockPrefs extends MockChoices {
	enabled: boolean;
	switcher: boolean;
}

export const MOCK_PREFS_DEFAULT: WatchOrderMockPrefs = {
	enabled: false,
	switcher: false,
	order: 'linked',
	strip: 'rail',
	placement: 'top',
	titles: 'full',
	upNext: 'poster',
	franchise: 'list',
	setting: 'segmented',
	handoff: 'next'
};
export const MOCK_PREFS_STORAGE_KEY = 'ani-gui:watch-order-mockups';

export const MOCK_OPTION_KEYS = Object.keys(MOCK_OPTIONS) as MockOptionKey[];

export function parseMockPrefs(raw: string | null): WatchOrderMockPrefs {
	void raw;
	return MOCK_PREFS_DEFAULT;
}

export function loadMockPrefs(storage: StorageLike | null): WatchOrderMockPrefs {
	void storage;
	return MOCK_PREFS_DEFAULT;
}

export function saveMockPrefs(storage: StorageLike | null, prefs: WatchOrderMockPrefs): void {
	void storage;
	void prefs;
}
