import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	ROW_PREFS_DEFAULT,
	ROW_PREFS_STORAGE_KEY,
	ROW_STYLES,
	loadRowPrefs,
	parseRowPrefs,
	saveRowPrefs,
	type RowPrefs
} from './prefs';
import { HERO_PREFS_STORAGE_KEY } from '$lib/hero/prefs';

class MemStorage {
	map = new Map<string, string>();
	getItem(k: string) {
		return this.map.get(k) ?? null;
	}
	setItem(k: string, v: string) {
		this.map.set(k, v);
	}
}
const broken = {
	getItem: () => {
		throw new Error('denied');
	},
	setItem: () => {
		throw new Error('denied');
	}
};

describe('the defaults', () => {
	it('put the actions inline, the switch hidden', () => {
		expect(ROW_PREFS_DEFAULT).toEqual({ style: 'inline', controls: false });
	});
	it('are kept under a key of their own, apart from the hero’s', () => {
		expect(ROW_PREFS_STORAGE_KEY).toBe('ani-gui:row-style');
		expect(ROW_PREFS_STORAGE_KEY).not.toBe(HERO_PREFS_STORAGE_KEY);
	});
});

describe('parseRowPrefs', () => {
	it('reads every field', () => {
		expect(parseRowPrefs('{"style":"band","controls":true}')).toEqual({
			style: 'band',
			controls: true
		});
	});
	it('falls back per field on unknown values', () => {
		expect(parseRowPrefs('{"style":"cards","controls":true}')).toEqual({
			style: 'inline',
			controls: true
		});
		expect(parseRowPrefs('{"style":"band","controls":"yes"}')).toEqual({
			style: 'band',
			controls: false
		});
	});
	it('takes the default for nothing stored or anything unreadable', () => {
		for (const raw of [null, '', 'not json', '7', '"band"', 'null', '[]']) {
			expect(parseRowPrefs(raw), String(raw)).toEqual(ROW_PREFS_DEFAULT);
		}
	});
});

describe('load and save', () => {
	it('round-trips through storage', () => {
		const s = new MemStorage();
		const p: RowPrefs = { style: 'band', controls: true };
		saveRowPrefs(s, p);
		expect(s.map.has(ROW_PREFS_STORAGE_KEY)).toBe(true);
		expect(loadRowPrefs(s)).toEqual(p);
	});
	it('loads the default with no storage, or storage that refuses', () => {
		expect(loadRowPrefs(null)).toEqual(ROW_PREFS_DEFAULT);
		expect(loadRowPrefs(broken)).toEqual(ROW_PREFS_DEFAULT);
	});
	it('saves quietly where storage refuses or is missing', () => {
		expect(() => saveRowPrefs(broken, ROW_PREFS_DEFAULT)).not.toThrow();
		expect(() => saveRowPrefs(null, ROW_PREFS_DEFAULT)).not.toThrow();
	});
});

describe('parseRowPrefs, by property', () => {
	it('always yields a known style and a boolean, whatever is stored', () => {
		fc.assert(
			fc.property(fc.oneof(fc.string(), fc.json(), fc.constant(null)), (raw) => {
				const p = parseRowPrefs(raw);
				expect(ROW_STYLES).toContain(p.style);
				expect(typeof p.controls).toBe('boolean');
			})
		);
	});

	it('reads back exactly what was saved', () => {
		fc.assert(
			fc.property(fc.constantFrom(...ROW_STYLES), fc.boolean(), (style, controls) => {
				const s = new MemStorage();
				saveRowPrefs(s, { style, controls });
				expect(loadRowPrefs(s)).toEqual({ style, controls });
			})
		);
	});
});
