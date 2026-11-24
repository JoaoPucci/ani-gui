/**
 * What the Debug page shows of the home hero's last pick: the walk the
 * backend's pick returned (`GET /api/home/hero/walk`, backend/src/hero),
 * read once when the page opens, and the words for what it holds. The
 * page renders it read-only; nothing here decides anything the pick
 * did, it only names it.
 */

import type { HeroArt, HeroVerdict, HeroWalkStep, HeroWalkView } from '$lib/api';
import { m } from '$lib/paraglide/messages';

/** Where the page's read of the walk stands. */
export type HeroWalkState =
	| { kind: 'loading' }
	| { kind: 'read'; view: HeroWalkView }
	| { kind: 'failed' };

/** Read the walk; failed when the backend cannot answer. */
export async function readHeroWalk(read: () => Promise<HeroWalkView>): Promise<HeroWalkState> {
	try {
		return { kind: 'read', view: await read() };
	} catch {
		return { kind: 'failed' };
	}
}

const ART: Record<HeroArt, () => string> = {
	banner: m.diagnostics_hero_walk_art_banner,
	cover: m.diagnostics_hero_walk_art_cover,
	none: m.diagnostics_hero_walk_art_none
};

/** The words for a show's art. */
export const artLabel = (art: HeroArt): string => ART[art]();

const TAKEN: Record<Extract<HeroVerdict, { kind: 'taken' }>['because'], () => string> = {
	art: m.diagnostics_hero_walk_taken_art,
	airing_art: m.diagnostics_hero_walk_taken_airing_art,
	fill: m.diagnostics_hero_walk_taken_fill
};

const SKIPPED: Record<Extract<HeroVerdict, { kind: 'skipped' }>['because'], () => string> = {
	repeat: m.diagnostics_hero_walk_skipped_repeat,
	not_airing: m.diagnostics_hero_walk_skipped_not_airing,
	hero_full: m.diagnostics_hero_walk_skipped_hero_full,
	no_art: m.diagnostics_hero_walk_skipped_no_art
};

/** What the pick did with a show, and why; slots counted from one. */
export function outcomeLabel(verdict: HeroVerdict): string {
	if (verdict.kind === 'taken') {
		return m.diagnostics_hero_walk_taken({
			slot: verdict.slot + 1,
			reason: TAKEN[verdict.because]()
		});
	}
	return m.diagnostics_hero_walk_skipped({ reason: SKIPPED[verdict.because]() });
}

const ORIGIN: Record<string, () => string> = {
	file: m.diagnostics_hero_walk_origin_file,
	cache: m.diagnostics_hero_walk_origin_cache,
	defaults: m.diagnostics_hero_walk_origin_defaults,
	test: m.diagnostics_hero_walk_origin_test,
	pending: m.diagnostics_hero_walk_origin_pending
};

/** Where the launch's flags came from; an origin this build does not
 *  know is shown as the backend named it. */
export function originLabel(origin: string): string {
	const label = Object.hasOwn(ORIGIN, origin) ? ORIGIN[origin] : undefined;
	return label ? label() : origin;
}

const STATUS: Record<string, () => string> = {
	finished: m.diagnostics_hero_walk_status_finished,
	upcoming: m.diagnostics_hero_walk_status_upcoming,
	unreleased: m.diagnostics_hero_walk_status_unreleased,
	tba: m.diagnostics_hero_walk_status_tba
};

/** A show's status: airing, or the name of Kitsu's status for it; a
 *  status this build does not know is shown as Kitsu names it. */
export function statusLabel(step: Pick<HeroWalkStep, 'status' | 'airing'>): string {
	if (step.airing) return m.diagnostics_hero_walk_status_airing();
	if (step.status === null) return m.diagnostics_hero_walk_status_unknown();
	const label = Object.hasOwn(STATUS, step.status) ? STATUS[step.status] : undefined;
	return label ? label() : step.status;
}
