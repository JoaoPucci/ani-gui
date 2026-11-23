import { describe, expect, it } from 'vitest';
import {
	loadStartLook,
	parseStartLook,
	saveStartLook,
	START_LOOK_DEFAULT,
	START_LOOK_STORAGE_KEY
} from './start-look';

function memory() {
	const map = new Map<string, string>();
	return {
		map,
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v)
	};
}
const refusing = {
	getItem: (): string | null => {
		throw new Error('denied');
	},
	setItem: () => {
		throw new Error('denied');
	}
};

describe('the start look', () => {
	it('is the episode still unless the title card was chosen', () => {
		expect(START_LOOK_DEFAULT).toBe('still');
		expect(parseStartLook(null)).toBe('still');
		expect(parseStartLook('card')).toBe('card');
		expect(parseStartLook('still')).toBe('still');
		expect(parseStartLook('hairline')).toBe('still');
		expect(parseStartLook('')).toBe('still');
	});

	it('is kept per machine, and a storage that refuses keeps the default', () => {
		const s = memory();
		saveStartLook(s, 'card');
		expect(s.map.get(START_LOOK_STORAGE_KEY)).toBe('card');
		expect(loadStartLook(s)).toBe('card');
		expect(loadStartLook(refusing)).toBe('still');
		expect(() => saveStartLook(refusing, 'card')).not.toThrow();
		expect(loadStartLook(null)).toBe('still');
	});
});
