import { describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';
import type { LadderLevel } from './quality-ladder';
import { levelLabel, qualityMenu, type QualityView } from './quality-menu';

const LEVELS: LadderLevel[] = [
	{ height: 480, bitrate: 800_000 },
	{ height: 720, bitrate: 2_500_000 },
	{ height: 1080, bitrate: 5_000_000 }
];

function view(over: Partial<QualityView> = {}): QualityView {
	return { levels: LEVELS, pinned: null, playing: null, unavailable: [], ...over };
}

describe('levelLabel', () => {
	it('names a level by its height', () => {
		expect(levelLabel(LEVELS, 2)).toBe(m.play_quality_height({ height: '1080' }));
		expect(levelLabel(LEVELS, 2)).toBe('1080p');
	});

	it('adds the bitrate when another level has the same height', () => {
		const twins: LadderLevel[] = [
			{ height: 1080, bitrate: 4_000_000 },
			{ height: 1080, bitrate: 6_250_000 }
		];
		expect(levelLabel(twins, 1)).toBe(
			m.play_quality_height_bitrate({ height: '1080', mbps: '6.3' })
		);
		expect(levelLabel(twins, 0)).toBe('1080p · 4.0 Mbps');
	});

	it('names a level with no height by its bitrate', () => {
		expect(levelLabel([{ height: 0, bitrate: 1_500_000 }], 0)).toBe(
			m.play_quality_bitrate({ mbps: '1.5' })
		);
	});
});

describe('qualityMenu', () => {
	it('has nothing to offer without a stream or with one level', () => {
		expect(qualityMenu(null)).toBeNull();
		expect(qualityMenu(view({ levels: [LEVELS[1]] }))).toBeNull();
	});

	it('lists Auto, then the levels highest first, with Auto checked', () => {
		const menu = qualityMenu(view());
		expect(menu?.items.map((i) => [i.key, i.label, i.checked])).toEqual([
			['auto', m.play_quality_auto(), true],
			[2, '1080p', false],
			[1, '720p', false],
			[0, '480p', false]
		]);
	});

	it('says what Auto is playing once a segment plays', () => {
		expect(qualityMenu(view())?.current).toBe(m.play_quality_auto());
		expect(qualityMenu(view({ playing: 2 }))?.current).toBe(
			m.play_quality_auto_playing({ quality: '1080p' })
		);
		expect(qualityMenu(view({ playing: 9 }))?.current).toBe(m.play_quality_auto());
	});

	it('checks a pinned level and names it as the row value', () => {
		const menu = qualityMenu(view({ pinned: 1, playing: 2 }));
		expect(menu?.items.filter((i) => i.checked).map((i) => i.key)).toEqual([1]);
		expect(menu?.current).toBe('720p');
	});

	it('leaves out the levels the engine could not load', () => {
		const menu = qualityMenu(view({ unavailable: [2] }));
		expect(menu?.items.map((i) => i.key)).toEqual(['auto', 1, 0]);
	});
});
