/**
 * Hero banner preferences, kept together per machine in local storage:
 *
 *   - `framing` — how the hero frames its image:
 *       `top`:     cover, anchored near the top, heavy fades — the framing
 *                  the heroes had before AniList's banner reached them
 *       `contain`: never crop — the whole image over a blurred copy of itself
 *       `pillar`:  fill the height — the whole upload at the band's height,
 *                  pillarboxed over its blurred copy when narrow, cropped at
 *                  the sides when wide
 *       `focus`:   cover anchored at the centre, fades only behind text
 *   - `source` — which image comes first: Kitsu's cover, or AniList's
 *     banner with Kitsu's cover as the fallback
 *   - `controls` — whether the on-page switch that changes these is
 *     shown; the diagnostics page owns it
 *
 * The heroes' default is `pillar` with `anilist`: the detail band is
 * always 4.8:1 and the home band changes shape with the window, so a
 * cover cropped to fill either loses a third of its height at every
 * size; filling the height shows the whole image, and AniList's banners
 * are drawn for a band that shape. Which image a hero renders from
 * these is `./framing`'s business.
 */

export type HeroFraming = 'top' | 'contain' | 'pillar' | 'focus';
export type HeroSource = 'kitsu' | 'anilist';

export interface HeroPrefs {
	framing: HeroFraming;
	source: HeroSource;
	controls: boolean;
}

export const HERO_PREFS_DEFAULT: HeroPrefs = {
	framing: 'pillar',
	source: 'anilist',
	controls: false
};
export const HERO_PREFS_STORAGE_KEY = 'ani-gui:hero-framing';

export const HERO_FRAMINGS: readonly HeroFraming[] = ['top', 'contain', 'pillar', 'focus'];
export const HERO_SOURCES: readonly HeroSource[] = ['kitsu', 'anilist'];

function framingOr(v: unknown, fallback: HeroFraming): HeroFraming {
	return HERO_FRAMINGS.find((f) => f === v) ?? fallback;
}
function sourceOr(v: unknown, fallback: HeroSource): HeroSource {
	return HERO_SOURCES.find((s) => s === v) ?? fallback;
}

/** Read stored preferences, falling back to the default per field. */
export function parseHeroPrefs(raw: string | null): HeroPrefs {
	let v: unknown;
	try {
		v = JSON.parse(raw ?? 'null');
	} catch {
		v = null;
	}
	if (typeof v !== 'object' || v === null) return HERO_PREFS_DEFAULT;
	const o = v as Record<string, unknown>;
	return {
		framing: framingOr(o.framing, HERO_PREFS_DEFAULT.framing),
		source: sourceOr(o.source, HERO_PREFS_DEFAULT.source),
		controls: o.controls === true
	};
}

export interface StorageLike {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

/** The window's local storage, or `null` where there is none or
 *  reaching it throws, as it does in a context that denies it. */
export function browserStorage(): StorageLike | null {
	try {
		return (globalThis as { localStorage?: StorageLike }).localStorage ?? null;
	} catch {
		return null;
	}
}

/** The stored preferences, or the default when there is no storage,
 *  storage refuses, or nothing is stored. */
export function loadHeroPrefs(storage: StorageLike | null): HeroPrefs {
	let stored: string | null;
	try {
		stored = storage?.getItem(HERO_PREFS_STORAGE_KEY) ?? null;
	} catch {
		stored = null;
	}
	return parseHeroPrefs(stored);
}

/** Keep `prefs` for the next load. A refusal is not an error: the
 *  caller already holds the preferences for this session. */
export function saveHeroPrefs(storage: StorageLike | null, prefs: HeroPrefs): void {
	try {
		storage?.setItem(HERO_PREFS_STORAGE_KEY, JSON.stringify(prefs));
	} catch {
		// Not persisted: this session still uses it.
	}
}
