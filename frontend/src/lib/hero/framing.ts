/**
 * Which image a hero renders, from the show's artwork and the stored
 * preferences (`./prefs`): AniList's banner first under the AniList
 * source, then Kitsu's cover, then the poster — passing over any URL
 * that has failed to load this session.
 */

import { SvelteSet } from 'svelte/reactivity';

import type { HeroPrefs } from './prefs';

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

function firstOf(
	bag: ImageBag | null,
	order: readonly Slot[],
	failed: ReadonlySet<string>
): string | null {
	if (!bag) return null;
	for (const slot of order) {
		const v = bag[slot];
		if (v != null && !failed.has(v)) return v;
	}
	return null;
}

const NO_FAILURES: ReadonlySet<string> = new Set();

const COVER_ORDER: Record<'home' | 'detail', readonly Slot[]> = {
	home: ['large', 'original', 'small'],
	detail: ['large', 'small', 'original']
};
const COVER_WHOLE: readonly Slot[] = ['original', 'large', 'small'];
const POSTER_ORDER: Record<'home' | 'detail', readonly Slot[]> = {
	home: ['large', 'original', 'medium'],
	detail: ['large', 'medium', 'original']
};

/** Image URLs that failed to load this session. A hero skips them;
 *  nothing retries one until the app is next started. */
export const failedHeroImages = new SvelteSet<string>();

/** Record that `url` failed to load, so no hero picks it again. */
export function noteHeroImageFailed(url: string): void {
	failedHeroImages.add(url);
}

/**
 * The image a hero renders. `surface` keeps each page's existing slot
 * order for the `top` and `focus` framings; `contain` and `pillar`
 * prefer the cover's `original`, the whole upload, since Kitsu's sized
 * covers are themselves centre crops of it. URLs in `failed` are
 * passed over for the next in order.
 */
export function pickHeroImage(
	d: HeroImageInput,
	prefs: Pick<HeroPrefs, 'framing' | 'source'>,
	surface: 'home' | 'detail',
	failed: ReadonlySet<string> = NO_FAILURES
): HeroPick {
	const banner = d.anilist_banner;
	if (prefs.source === 'anilist' && banner && !failed.has(banner)) {
		return { url: banner, isCover: true, origin: 'anilist' };
	}
	const cover = firstOf(
		d.cover_image,
		prefs.framing === 'contain' || prefs.framing === 'pillar' ? COVER_WHOLE : COVER_ORDER[surface],
		failed
	);
	if (cover !== null) {
		// The backend fills a null Kitsu cover with AniList's banner.
		const origin = cover === banner ? 'anilist' : 'kitsu';
		return { url: cover, isCover: true, origin };
	}
	const poster = firstOf(d.poster_image, POSTER_ORDER[surface], failed);
	if (poster !== null) return { url: poster, isCover: false, origin: 'poster' };
	return { url: null, isCover: false, origin: null };
}

/** Natural aspect ratio of a loaded image, or `null` when unknown. */
export function naturalRatio(width: number, height: number): number | null {
	return width > 0 && height > 0 ? width / height : null;
}
