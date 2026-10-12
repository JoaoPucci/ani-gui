/**
 * Hero banner framing. Decides which image a hero renders and how it
 * is framed:
 *
 *   - `top`:     cover, anchored near the top, heavy fades — the framing
 *                the heroes had before AniList's banner reached them
 *   - `contain`: never crop — the whole image over a blurred copy of itself
 *   - `pillar`:  fill the height — the whole upload at the band's height,
 *                pillarboxed over its blurred copy when narrow, cropped at
 *                the sides when wide
 *   - `focus`:   cover anchored at the centre, fades only behind text
 *
 * and which image comes first: Kitsu's cover, or AniList's banner with
 * Kitsu's cover as the fallback. The heroes' default is `pillar` with
 * `anilist`: the detail band is always 4.8:1 and the home band changes
 * shape with the window, so a cover cropped to fill either loses a
 * third of its height at every size; filling the height shows the
 * whole image, and AniList's banners are drawn for a band that shape.
 * `controls` says whether the on-page switch that changes these is
 * shown; the diagnostics page owns it. The three are kept together
 * per machine, in local storage.
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

export type HeroOrigin = 'anilist' | 'kitsu' | 'poster';

export interface HeroPick {
	url: string | null;
	isCover: boolean;
	origin: HeroOrigin | null;
}

interface ImageBag {
	tiny?: string | null;
	small?: string | null;
	medium?: string | null;
	large?: string | null;
	original?: string | null;
}

export interface HeroImageInput {
	anilist_banner?: string | null;
	cover_image: ImageBag | null;
	poster_image: ImageBag | null;
}

type Slot = keyof ImageBag;

function firstOf(bag: ImageBag | null, order: readonly Slot[]): string | null {
	if (!bag) return null;
	for (const slot of order) {
		const v = bag[slot];
		if (v != null) return v;
	}
	return null;
}

const COVER_ORDER: Record<'home' | 'detail', readonly Slot[]> = {
	home: ['large', 'original', 'small'],
	detail: ['large', 'small', 'original']
};
const COVER_WHOLE: readonly Slot[] = ['original', 'large', 'small'];
const POSTER_ORDER: Record<'home' | 'detail', readonly Slot[]> = {
	home: ['large', 'original', 'medium'],
	detail: ['large', 'medium', 'original']
};

/**
 * The image a hero renders. `surface` keeps each page's existing slot
 * order for the `top` and `focus` framings; `contain` and `pillar`
 * prefer the cover's `original`, the whole upload, since Kitsu's sized
 * covers are themselves centre crops of it.
 */
export function pickHeroImage(
	d: HeroImageInput,
	prefs: Pick<HeroPrefs, 'framing' | 'source'>,
	surface: 'home' | 'detail'
): HeroPick {
	if (prefs.source === 'anilist' && d.anilist_banner) {
		return { url: d.anilist_banner, isCover: true, origin: 'anilist' };
	}
	const cover = firstOf(
		d.cover_image,
		prefs.framing === 'contain' || prefs.framing === 'pillar' ? COVER_WHOLE : COVER_ORDER[surface]
	);
	if (cover !== null) {
		// The backend fills a null Kitsu cover with AniList's banner.
		const origin = cover === d.anilist_banner ? 'anilist' : 'kitsu';
		return { url: cover, isCover: true, origin };
	}
	const poster = firstOf(d.poster_image, POSTER_ORDER[surface]);
	if (poster !== null) return { url: poster, isCover: false, origin: 'poster' };
	return { url: null, isCover: false, origin: null };
}

/** Natural aspect ratio of a loaded image, or `null` when unknown. */
export function naturalRatio(width: number, height: number): number | null {
	return width > 0 && height > 0 ? width / height : null;
}
