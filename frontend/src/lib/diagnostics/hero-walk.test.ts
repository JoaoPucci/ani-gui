import { describe, expect, it, vi } from 'vitest';

import { artLabel, originLabel, outcomeLabel, readHeroWalk, statusLabel } from './hero-walk';
import {
	HERO_ARTS,
	HERO_SKIPPED_BECAUSE,
	HERO_TAKEN_BECAUSE,
	type HeroWalkStep,
	type HeroWalkView
} from '$lib/api';
import { m } from '$lib/paraglide/messages';

const view: HeroWalkView = {
	flag: { name: 'hero_airing_first', on: true, origin: 'file' },
	walk: { airing_first: true, count: 3, entries: 0, steps: [] }
};

describe('readHeroWalk', () => {
	it('is the walk the backend answers', async () => {
		expect(await readHeroWalk(async () => view)).toEqual({ kind: 'read', view });
	});

	it('is failed when the backend cannot answer', async () => {
		const read = vi.fn(async (): Promise<HeroWalkView> => {
			throw new Error('down');
		});
		expect(await readHeroWalk(read)).toEqual({ kind: 'failed' });
	});
});

describe('the walk labels', () => {
	it('names every kind of art apart', () => {
		const labels = HERO_ARTS.map(artLabel);
		expect(new Set(labels).size).toBe(HERO_ARTS.length);
		expect(artLabel('banner')).toBe(m.diagnostics_hero_walk_art_banner());
		expect(artLabel('none')).toBe(m.diagnostics_hero_walk_art_none());
	});

	it('says, for a show taken, which slot it took and why, counting slots from one', () => {
		const reasons = HERO_TAKEN_BECAUSE.map((because) =>
			outcomeLabel({ kind: 'taken', slot: 0, because })
		);
		expect(new Set(reasons).size).toBe(HERO_TAKEN_BECAUSE.length);
		expect(outcomeLabel({ kind: 'taken', slot: 1, because: 'airing_art' })).toBe(
			m.diagnostics_hero_walk_taken({
				slot: 2,
				reason: m.diagnostics_hero_walk_taken_airing_art()
			})
		);
	});

	it('says, for a show skipped, why', () => {
		const reasons = HERO_SKIPPED_BECAUSE.map((because) =>
			outcomeLabel({ kind: 'skipped', because })
		);
		expect(new Set(reasons).size).toBe(HERO_SKIPPED_BECAUSE.length);
		expect(outcomeLabel({ kind: 'skipped', because: 'not_airing' })).toBe(
			m.diagnostics_hero_walk_skipped({ reason: m.diagnostics_hero_walk_skipped_not_airing() })
		);
	});

	it('names where the flags came from, and an origin it does not know as it is', () => {
		const known = ['file', 'cache', 'defaults', 'test', 'pending'].map(originLabel);
		expect(new Set(known).size).toBe(5);
		expect(originLabel('defaults')).toBe(m.diagnostics_hero_walk_origin_defaults());
		expect(originLabel('something-new')).toBe('something-new');
	});

	it('says airing for an airing show, and names the status Kitsu gave otherwise', () => {
		const step = (status: string | null, airing: boolean) =>
			({ status, airing }) as Pick<HeroWalkStep, 'status' | 'airing'>;
		expect(statusLabel(step('current', true))).toBe(m.diagnostics_hero_walk_status_airing());
		expect(statusLabel(step('finished', false))).toBe(m.diagnostics_hero_walk_status_finished());
		expect(statusLabel(step('upcoming', false))).toBe(m.diagnostics_hero_walk_status_upcoming());
		expect(statusLabel(step('unreleased', false))).toBe(
			m.diagnostics_hero_walk_status_unreleased()
		);
		expect(statusLabel(step('tba', false))).toBe(m.diagnostics_hero_walk_status_tba());
		// A status Kitsu adds later is shown as Kitsu names it.
		expect(statusLabel(step('hiatus', false))).toBe('hiatus');
		expect(statusLabel(step(null, false))).toBe(m.diagnostics_hero_walk_status_unknown());
	});
});
