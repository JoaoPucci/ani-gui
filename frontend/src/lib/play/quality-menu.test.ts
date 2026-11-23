import * as fc from 'fast-check';
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
	});

	it('adds the bitrate when another level has the same height', () => {
		const twins: LadderLevel[] = [
			{ height: 1080, bitrate: 4_000_000 },
			{ height: 1080, bitrate: 6_250_000 }
		];
		expect(levelLabel(twins, 1)).toBe(
			m.play_quality_height_bitrate({ height: '1080', mbps: '6.3' })
		);
		expect(levelLabel(twins, 0)).toBe(
			m.play_quality_height_bitrate({ height: '1080', mbps: '4.0' })
		);
	});

	it('names a level with no height by its bitrate', () => {
		expect(levelLabel([{ height: 0, bitrate: 1_500_000 }], 0)).toBe(
			m.play_quality_bitrate({ mbps: '1.5' })
		);
	});
});

describe('qualityMenu', () => {
	it('has nothing to show without a stream or before a level', () => {
		expect(qualityMenu(null)).toBeNull();
		expect(qualityMenu(view({ levels: [] }))).toBeNull();
	});

	it('shows a stream of one level as that level, checked, with no Auto', () => {
		const one = [LEVELS[2]];
		for (const over of [
			{},
			{ playing: 0 },
			// A pin carried from an earlier stream can land on the one level.
			{ pinned: 0, playing: 0 }
		]) {
			const menu = qualityMenu(view({ levels: one, ...over }));
			expect(menu?.items.map((i) => [i.key, i.label, i.checked])).toEqual([
				[0, m.play_quality_height({ height: '1080' }), true]
			]);
			expect(menu?.current).toBe(m.play_quality_height({ height: '1080' }));
		}
	});

	it('names a lone level with no height by its bitrate', () => {
		const menu = qualityMenu(view({ levels: [{ height: 0, bitrate: 1_500_000 }] }));
		expect(menu?.current).toBe(m.play_quality_bitrate({ mbps: '1.5' }));
		expect(menu?.items.map((i) => i.label)).toEqual([m.play_quality_bitrate({ mbps: '1.5' })]);
	});

	it('waits to show a lone level until something names it', () => {
		// A media playlist served alone lists neither a height nor a
		// bandwidth; the row appears once a parsed segment gives its height.
		expect(qualityMenu(view({ levels: [{ height: 0, bitrate: 0 }] }))).toBeNull();
		expect(qualityMenu(view({ levels: [{ height: 1080, bitrate: 0 }] }))?.current).toBe(
			m.play_quality_height({ height: '1080' })
		);
	});

	it('shows nothing for a lone level the engine could not load', () => {
		expect(qualityMenu(view({ levels: [LEVELS[0]], unavailable: [0] }))).toBeNull();
	});

	it('lists Auto, then the levels highest first, with Auto checked', () => {
		const menu = qualityMenu(view());
		expect(menu?.items.map((i) => [i.key, i.label, i.checked])).toEqual([
			['auto', m.play_quality_auto(), true],
			[2, m.play_quality_height({ height: '1080' }), false],
			[1, m.play_quality_height({ height: '720' }), false],
			[0, m.play_quality_height({ height: '480' }), false]
		]);
	});

	it('says what Auto is playing once a segment plays', () => {
		expect(qualityMenu(view())?.current).toBe(m.play_quality_auto());
		expect(qualityMenu(view({ playing: 2 }))?.current).toBe(
			m.play_quality_auto_playing({ quality: m.play_quality_height({ height: '1080' }) })
		);
		expect(qualityMenu(view({ playing: 9 }))?.current).toBe(m.play_quality_auto());
	});

	it('checks a pinned level and names it as the row value', () => {
		// The play never sits above its pin: a pinned play plays its pin
		// or a level below it.
		const menu = qualityMenu(view({ pinned: 1, playing: 1 }));
		expect(menu?.items.filter((i) => i.checked).map((i) => i.key)).toEqual([1]);
		expect(menu?.current).toBe(m.play_quality_height({ height: '720' }));
	});

	it('names what plays under a pin while the play sits below it', () => {
		expect(qualityMenu(view({ pinned: 2, playing: 1 }))?.current).toBe(
			m.play_quality_pinned_playing({
				pinned: m.play_quality_height({ height: '1080' }),
				quality: m.play_quality_height({ height: '720' })
			})
		);
		expect(qualityMenu(view({ pinned: 2, playing: 2 }))?.current).toBe(
			m.play_quality_height({ height: '1080' })
		);
	});

	it('leaves out the levels the engine could not load', () => {
		const menu = qualityMenu(view({ unavailable: [2] }));
		expect(menu?.items.map((i) => i.key)).toEqual(['auto', 1, 0]);
	});
});

const level = fc.record({
	height: fc.oneof(fc.constant(0), fc.integer({ min: 144, max: 2160 })),
	bitrate: fc.oneof(fc.constant(0), fc.integer({ min: 1, max: 20_000_000 }))
});

describe('qualityMenu — properties', () => {
	it('one level gives one checked item and no Auto, once it can be named and loaded', () => {
		fc.assert(
			fc.property(
				level,
				fc.boolean(),
				fc.constantFrom(null, 0),
				fc.constantFrom(null, 0),
				(only, failed, pinned, playing) => {
					const menu = qualityMenu(
						view({ levels: [only], unavailable: failed ? [0] : [], pinned, playing })
					);
					const named = only.height > 0 || only.bitrate > 0;
					if (failed || !named) {
						expect(menu).toBeNull();
						return;
					}
					expect(menu?.items).toEqual([{ key: 0, label: levelLabel([only], 0), checked: true }]);
					expect(menu?.current).toBe(levelLabel([only], 0));
				}
			)
		);
	});

	it('two or more levels offer Auto, then the loadable levels highest first, with the choice checked', () => {
		const stream = fc
			.array(level, { minLength: 2, maxLength: 6 })
			.chain((levels) =>
				fc.record({
					levels: fc.constant(levels),
					unavailable: fc.subarray(levels.map((_, i) => i)),
					playing: fc.option(fc.nat({ max: levels.length - 1 }), { nil: null })
				})
			)
			.chain((s) => {
				const loadable = s.levels.map((_, i) => i).filter((i) => !s.unavailable.includes(i));
				const pin =
					loadable.length > 0
						? fc.option(fc.constantFrom(...loadable), { nil: null })
						: fc.constant(null);
				return pin.map((pinned) => ({ ...s, pinned }));
			});
		const rank = ({ height, bitrate }: LadderLevel) => [height, bitrate];
		const above = (a: LadderLevel, b: LadderLevel) => {
			const [ha, ba] = rank(a);
			const [hb, bb] = rank(b);
			return ha > hb || (ha === hb && ba >= bb);
		};
		fc.assert(
			fc.property(stream, (s) => {
				const menu = qualityMenu(view(s));
				const keys = menu?.items.map((i) => i.key) ?? [];
				// Auto, then every loadable level once, each no lower than the next.
				expect(keys[0]).toBe('auto');
				const offered = keys.slice(1) as number[];
				expect([...offered].sort((a, b) => a - b)).toEqual(
					s.levels.map((_, i) => i).filter((i) => !s.unavailable.includes(i))
				);
				for (let i = 1; i < offered.length; i++) {
					expect(above(s.levels[offered[i - 1]], s.levels[offered[i]])).toBe(true);
				}
				expect(menu?.items.filter((i) => i.checked).map((i) => i.key)).toEqual([
					s.pinned ?? 'auto'
				]);
				const auto =
					s.playing === null
						? m.play_quality_auto()
						: m.play_quality_auto_playing({ quality: levelLabel(s.levels, s.playing) });
				const pinnedCurrent =
					s.pinned === null
						? auto
						: s.playing === null || s.playing === s.pinned
							? levelLabel(s.levels, s.pinned)
							: m.play_quality_pinned_playing({
									pinned: levelLabel(s.levels, s.pinned),
									quality: levelLabel(s.levels, s.playing)
								});
				expect(menu?.current).toBe(pinnedCurrent);
			})
		);
	});
});
