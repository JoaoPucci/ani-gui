// A fake of the backend's home hero pick (`POST /api/home/hero`,
// backend/src/hero `pick`), for the renderer's tests: the acceptance
// tier answers Home's pick with it, under the airing preference a
// scenario names, so a scenario reads the same as it would against the
// app. `tests/acceptance/hero-pick-fake.test.ts` holds it to the
// backend's golden cases (backend/tests/fixtures/hero-pick/cases.json),
// which the backend's own test checks against `pick`.

import type {
	HeroArt,
	HeroPick,
	HeroShow,
	HeroSkippedBecause,
	HeroTakenBecause,
	HeroVerdict,
	HeroWalkStep
} from '../lib/api';

/** `HERO_ROTATION_COUNT`. */
export const HERO_COUNT = 3;

const real = (v: string | null | undefined): boolean => typeof v === 'string' && v.length > 0;

/** `art`. */
export function heroArt(show: HeroShow): HeroArt {
	if (real(show.anilist_banner)) return 'banner';
	const c = show.cover_image;
	return c && (real(c.large) || real(c.original) || real(c.small)) ? 'cover' : 'none';
}

/** `is_airing`. */
export const isAiring = (show: HeroShow): boolean => show.status === 'current';

/** `pick`. */
export function heroPick(
	shows: readonly HeroShow[],
	airingFirst: boolean,
	count = HERO_COUNT
): HeroPick {
	const seen = new Set<string>();
	const first = shows.map((s) => !seen.has(s.id) && Boolean(seen.add(s.id)));
	const arts = shows.map(heroArt);
	const firsts = shows.map((_, i) => i).filter((i) => first[i]);
	const hasArt = (i: number) => arts[i] !== 'none';
	const lifted = (i: number, on: boolean) => on && isAiring(shows[i]);
	const artOrder = (on: boolean) => {
		const withArt = firsts.filter(hasArt);
		return [...withArt.filter((i) => lifted(i, on)), ...withArt.filter((i) => !lifted(i, on))];
	};
	const hero = artOrder(airingFirst).slice(0, count);
	hero.push(...firsts.filter((i) => !hasArt(i)).slice(0, Math.max(0, count - hero.length)));
	const offArt = artOrder(false).slice(0, count);
	const artFull = hero.filter(hasArt).length === count;

	const taken = (i: number): HeroTakenBecause =>
		!hasArt(i) ? 'fill' : lifted(i, airingFirst) ? 'airing_art' : 'art';
	const skipped = (i: number): HeroSkippedBecause => {
		if (!hasArt(i)) return artFull ? 'no_art' : 'hero_full';
		return airingFirst && !lifted(i, true) && offArt.includes(i) ? 'not_airing' : 'hero_full';
	};
	const verdict = (i: number): HeroVerdict => {
		if (!first[i]) return { kind: 'skipped', because: 'repeat' };
		const slot = hero.indexOf(i);
		if (slot >= 0) return { kind: 'taken', slot, because: taken(i) };
		return { kind: 'skipped', because: skipped(i) };
	};
	const settling = firsts.filter((i) => hasArt(i) && (!airingFirst || lifted(i, true)));
	const walkLen =
		count === 0 ? 0 : settling.length >= count ? settling[count - 1] + 1 : shows.length;
	const steps: HeroWalkStep[] = shows.slice(0, walkLen).map((show, i) => {
		const v = verdict(i);
		return {
			rank: i + 1,
			id: show.id,
			title: show.title ?? '',
			art: arts[i],
			status: show.status ?? null,
			airing: isAiring(show),
			preference_applied:
				(v.kind === 'taken' && v.because === 'airing_art') ||
				(v.kind === 'skipped' && v.because === 'not_airing'),
			verdict: v
		};
	});
	return {
		hero: hero.map((i) => shows[i].id),
		row: firsts.filter((i) => !hero.includes(i)).map((i) => shows[i].id),
		walk: { airing_first: airingFirst, count, entries: shows.length, steps }
	};
}
