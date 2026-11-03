/**
 * Mini-player mockup preferences (0.21.0 design review), kept per
 * machine in local storage like the hero framing switch's
 * ($lib/hero/prefs). The diagnostics page owns `enabled` and
 * `switcher`; the mockup's own switch sets the rest. `platform`
 * stands in for the desktop the real window would run on, which a
 * single in-page mockup cannot be.
 */
import type { StorageLike } from '$lib/hero/prefs';

export const MINI_MOCK_OPTIONS = {
	layout: ['bar', 'centre'],
	main: ['back', 'placeholder'],
	onTop: ['card', 'pin', 'both'],
	platform: ['wayland', 'windows', 'x11'],
	gap: ['0', '1500', '4000'],
	cues: ['native', 'floor'],
	lift: ['lift', 'stay'],
	grip: ['chrome', 'always'],
	failure: ['inline', 'hand-back'],
	shortcut: ['i', 'none']
} as const;

type Options = typeof MINI_MOCK_OPTIONS;
export type MiniMockOptionKey = keyof Options;
export type MiniMockChoices = { [K in MiniMockOptionKey]: Options[K][number] };

export interface MiniMockPrefs extends MiniMockChoices {
	enabled: boolean;
	switcher: boolean;
	/** The one-time "keep it on top" card was dismissed. */
	hintSeen: boolean;
}

export const MINI_MOCK_PREFS_DEFAULT: MiniMockPrefs = {
	enabled: false,
	switcher: false,
	hintSeen: false,
	layout: 'bar',
	main: 'back',
	onTop: 'both',
	platform: 'wayland',
	gap: '1500',
	cues: 'floor',
	lift: 'lift',
	grip: 'chrome',
	failure: 'inline',
	shortcut: 'i'
};
export const MINI_MOCK_PREFS_STORAGE_KEY = 'ani-gui:mini-player-mockups';

export const MINI_MOCK_OPTION_KEYS = Object.keys(MINI_MOCK_OPTIONS) as MiniMockOptionKey[];

/** Stored text read as prefs; anything unknown falls back to the default. */
export function parseMiniMockPrefs(raw: string | null): MiniMockPrefs {
	void raw;
	return MINI_MOCK_PREFS_DEFAULT;
}

export function loadMiniMockPrefs(storage: StorageLike | null): MiniMockPrefs {
	void storage;
	return MINI_MOCK_PREFS_DEFAULT;
}

export function saveMiniMockPrefs(storage: StorageLike | null, prefs: MiniMockPrefs): void {
	void storage;
	void prefs;
}
