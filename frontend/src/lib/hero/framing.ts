/**
 * Hero banner framing EXPERIMENT (dev builds only). Decides which image
 * a hero renders and how it is framed, for side-by-side comparison of:
 *
 *   - `current`: master's rendering (cover, anchored near the top, heavy fades)
 *   - `contain`: never crop — the whole image over a blurred copy of itself
 *   - `focus`:   cover anchored at the centre, fades only behind text
 *
 * and of the image source: Kitsu's cover, or AniList's banner first with
 * Kitsu's cover as the fallback. Remove this module, its switch and the
 * branches that read it before any real change ships.
 */

export type HeroFraming = 'current' | 'contain' | 'focus';
export type HeroSource = 'kitsu' | 'anilist';

export interface HeroExperiment {
	framing: HeroFraming;
	source: HeroSource;
}

export const HERO_EXPERIMENT_DEFAULT: HeroExperiment = { framing: 'current', source: 'kitsu' };
export const HERO_EXPERIMENT_STORAGE_KEY = 'ani-gui:dev:hero-experiment';

/** Read a stored experiment, falling back to the default per field. */
export function parseHeroExperiment(raw: string | null): HeroExperiment {
	void raw;
	return HERO_EXPERIMENT_DEFAULT;
}

/** Apply `?hero=` / `?heroSrc=` over `base`; `null` when neither is present. */
export function heroExperimentFromQuery(
	search: string,
	base: HeroExperiment
): HeroExperiment | null {
	void search;
	void base;
	return null;
}

interface StorageLike {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

/** The experiment in force: the query when it names one (and it is
 *  persisted, so navigation keeps it), else the stored one. */
export function loadHeroExperiment(search: string, storage: StorageLike | null): HeroExperiment {
	void search;
	void storage;
	return HERO_EXPERIMENT_DEFAULT;
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

/**
 * The image a hero renders. `surface` keeps each page's existing slot
 * order for the `current` and `focus` framings; `contain` prefers the
 * cover's `original`, the whole upload, since Kitsu's sized covers are
 * themselves centre crops of it.
 */
export function pickHeroImage(
	d: HeroImageInput,
	exp: HeroExperiment,
	surface: 'home' | 'detail'
): HeroPick {
	void d;
	void exp;
	void surface;
	return { url: null, isCover: false, origin: null };
}

/** Natural aspect ratio of a loaded image, or `null` when unknown. */
export function naturalRatio(width: number, height: number): number | null {
	void width;
	void height;
	return null;
}
