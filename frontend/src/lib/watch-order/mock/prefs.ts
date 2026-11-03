/**
 * Watch-order mockup preferences, kept per machine in local storage
 * like the hero framing switch's ($lib/hero/prefs). The diagnostics
 * page owns `enabled` and `switcher`; the on-page switch sets the
 * rest. `order` stands in for the app setting the feature will add.
 */
import type { StorageLike } from '$lib/hero/prefs';
import { STRIP_WORDINGS } from '../strip-wording';

export const MOCK_OPTIONS = {
	order: ['linked', 'released'],
	strip: ['rail', 'trio'],
	placement: ['top', 'bottom'],
	titles: ['full', 'short'],
	upNext: ['poster', 'still'],
	franchise: ['list', 'shelf'],
	setting: ['segmented', 'radio'],
	handoff: ['next', 'episode'],
	wording: STRIP_WORDINGS,
	cta: ['button', 'stack'],
	resume: ['chip', 'sentence'],
	stackHover: ['surface', 'underline', 'border'],
	footer: ['center', 'axis', 'ends'],
	stackPosters: ['3', '2', '1', '0']
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
	strip: 'trio',
	placement: 'top',
	titles: 'full',
	upNext: 'poster',
	franchise: 'list',
	setting: 'segmented',
	handoff: 'next',
	wording: 'role',
	cta: 'stack',
	resume: 'chip',
	stackHover: 'surface',
	footer: 'center',
	stackPosters: '3'
};
export const MOCK_PREFS_STORAGE_KEY = 'ani-gui:watch-order-mockups';

export const MOCK_OPTION_KEYS = Object.keys(MOCK_OPTIONS) as MockOptionKey[];

export function parseMockPrefs(raw: string | null): WatchOrderMockPrefs {
	let v: unknown;
	try {
		v = JSON.parse(raw ?? 'null');
	} catch {
		v = null;
	}
	if (typeof v !== 'object' || v === null) return MOCK_PREFS_DEFAULT;
	const o = v as Record<string, unknown>;
	const out: Record<string, unknown> = {
		enabled: o.enabled === true,
		switcher: o.switcher === true
	};
	for (const key of MOCK_OPTION_KEYS) {
		const allowed: readonly string[] = MOCK_OPTIONS[key];
		out[key] = allowed.find((a) => a === o[key]) ?? MOCK_PREFS_DEFAULT[key];
	}
	return out as unknown as WatchOrderMockPrefs;
}

export function loadMockPrefs(storage: StorageLike | null): WatchOrderMockPrefs {
	let stored: string | null;
	try {
		stored = storage?.getItem(MOCK_PREFS_STORAGE_KEY) ?? null;
	} catch {
		stored = null;
	}
	return parseMockPrefs(stored);
}

export function saveMockPrefs(storage: StorageLike | null, prefs: WatchOrderMockPrefs): void {
	try {
		storage?.setItem(MOCK_PREFS_STORAGE_KEY, JSON.stringify(prefs));
	} catch {
		// Not persisted: this session still uses it.
	}
}
