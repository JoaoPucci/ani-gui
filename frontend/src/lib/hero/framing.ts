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

export type HeroFraming = 'current' | 'contain' | 'pillar' | 'focus';
export type HeroSource = 'kitsu' | 'anilist';

export interface HeroExperiment {
	framing: HeroFraming;
	source: HeroSource;
}

export const HERO_EXPERIMENT_DEFAULT: HeroExperiment = { framing: 'current', source: 'kitsu' };
export const HERO_EXPERIMENT_STORAGE_KEY = 'ani-gui:dev:hero-experiment';

const FRAMINGS: readonly HeroFraming[] = ['current', 'contain', 'focus'];
const SOURCES: readonly HeroSource[] = ['kitsu', 'anilist'];

function framingOr(v: unknown, fallback: HeroFraming): HeroFraming {
	return FRAMINGS.find((f) => f === v) ?? fallback;
}
function sourceOr(v: unknown, fallback: HeroSource): HeroSource {
	return SOURCES.find((s) => s === v) ?? fallback;
}

/** Read a stored experiment, falling back to the default per field. */
export function parseHeroExperiment(raw: string | null): HeroExperiment {
	let v: unknown;
	try {
		v = JSON.parse(raw ?? 'null');
	} catch {
		v = null;
	}
	if (typeof v !== 'object' || v === null) return HERO_EXPERIMENT_DEFAULT;
	const o = v as Record<string, unknown>;
	return {
		framing: framingOr(o.framing, HERO_EXPERIMENT_DEFAULT.framing),
		source: sourceOr(o.source, HERO_EXPERIMENT_DEFAULT.source)
	};
}

/** Apply `?hero=` / `?heroSrc=` over `base`; `null` when neither is present. */
export function heroExperimentFromQuery(
	search: string,
	base: HeroExperiment
): HeroExperiment | null {
	const q = new URLSearchParams(search);
	const hero = q.get('hero');
	const src = q.get('heroSrc');
	if (hero === null && src === null) return null;
	if (hero === 'reset') return HERO_EXPERIMENT_DEFAULT;
	return { framing: framingOr(hero, base.framing), source: sourceOr(src, base.source) };
}

interface StorageLike {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

/** The experiment in force: the query when it names one (and it is
 *  persisted, so navigation keeps it), else the stored one. */
export function loadHeroExperiment(search: string, storage: StorageLike | null): HeroExperiment {
	let stored: string | null;
	try {
		stored = storage?.getItem(HERO_EXPERIMENT_STORAGE_KEY) ?? null;
	} catch {
		stored = null;
	}
	const base = parseHeroExperiment(stored);
	const fromQuery = heroExperimentFromQuery(search, base);
	if (fromQuery === null) return base;
	try {
		storage?.setItem(HERO_EXPERIMENT_STORAGE_KEY, JSON.stringify(fromQuery));
	} catch {
		// Not persisted: this load still uses it.
	}
	return fromQuery;
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
		if (v) return v;
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
 * order for the `current` and `focus` framings; `contain` prefers the
 * cover's `original`, the whole upload, since Kitsu's sized covers are
 * themselves centre crops of it.
 */
export function pickHeroImage(
	d: HeroImageInput,
	exp: HeroExperiment,
	surface: 'home' | 'detail'
): HeroPick {
	if (exp.source === 'anilist' && d.anilist_banner) {
		return { url: d.anilist_banner, isCover: true, origin: 'anilist' };
	}
	const cover = firstOf(
		d.cover_image,
		exp.framing === 'contain' ? COVER_WHOLE : COVER_ORDER[surface]
	);
	if (cover) return { url: cover, isCover: true, origin: 'kitsu' };
	const poster = firstOf(d.poster_image, POSTER_ORDER[surface]);
	if (poster) return { url: poster, isCover: false, origin: 'poster' };
	return { url: null, isCover: false, origin: null };
}

/** Natural aspect ratio of a loaded image, or `null` when unknown. */
export function naturalRatio(width: number, height: number): number | null {
	return width > 0 && height > 0 ? width / height : null;
}
