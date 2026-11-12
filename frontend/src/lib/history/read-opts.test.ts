import { describe, expect, it } from 'vitest';
import { readOpts } from './read-opts';

describe('readOpts', () => {
	it('hands a read the signal it was given', () => {
		const page = new AbortController();
		expect(readOpts({ signal: page.signal })).toEqual([{ signal: page.signal }]);
	});

	it('hands a read no argument at all without a signal', () => {
		expect(readOpts({})).toEqual([]);
	});
});
