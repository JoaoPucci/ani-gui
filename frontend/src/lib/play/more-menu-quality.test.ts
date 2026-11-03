import * as fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import type { QualityMenu, QualityMenuItem } from './quality-menu';
import { moreMenuEscape, moreMenuQuality } from './more-menu-quality';

const LEVELS: QualityMenu = {
	items: [
		{ key: 'auto', label: 'Auto', checked: true },
		{ key: 2, label: '1080p', checked: false },
		{ key: 1, label: '720p', checked: false },
		{ key: 0, label: '360p', checked: false }
	],
	current: 'Auto (1080p)'
};

const LONE: QualityMenu = {
	items: [{ key: 0, label: '1080p', checked: true }],
	current: '1080p'
};

const item: fc.Arbitrary<QualityMenuItem> = fc.record({
	key: fc.oneof(fc.constant('auto' as const), fc.nat(8)),
	label: fc.string(),
	checked: fc.boolean()
});
const menu: fc.Arbitrary<QualityMenu> = fc.record({
	items: fc.array(item, { minLength: 1, maxLength: 8 }),
	current: fc.string()
});

describe('moreMenuQuality', () => {
	it('offers the quality menu as it stands while the custom controls are off', () => {
		const row = moreMenuQuality(LEVELS, false);
		expect(row?.menu).toBe(LEVELS);
		expect(row?.choosable).toBe(true);
	});

	it('has nothing to choose on a stream of one level, and still names it', () => {
		const row = moreMenuQuality(LONE, false);
		expect(row?.menu).toBe(LONE);
		expect(row?.choosable).toBe(false);
	});

	it('offers nothing while the custom controls carry the picker', () => {
		expect(moreMenuQuality(LEVELS, true)).toBeNull();
		expect(moreMenuQuality(LONE, true)).toBeNull();
	});

	it('offers nothing while the quality menu has nothing to show', () => {
		expect(moreMenuQuality(null, false)).toBeNull();
		expect(moreMenuQuality(null, true)).toBeNull();
	});

	it('passes any menu through unchanged, choosable exactly when it lists more than one entry', () => {
		fc.assert(
			fc.property(menu, (m) => {
				const row = moreMenuQuality(m, false);
				expect(row?.menu).toBe(m);
				expect(row?.choosable).toBe(m.items.length > 1);
				expect(moreMenuQuality(m, true)).toBeNull();
			})
		);
	});
});

describe('moreMenuEscape', () => {
	it('leaves the list of levels for the main list', () => {
		expect(moreMenuEscape(true)).toBe('back');
	});

	it('closes the menu from its main list', () => {
		expect(moreMenuEscape(false)).toBe('close');
	});
});
