import { describe, expect, it } from 'vitest';
import {
	MAX_REMEMBERED,
	keepPublished,
	parseMemory,
	remember,
	serializeMemory,
	type PublishedMemory
} from './published-memory';
import { raise, type Notice, type RaiseInput } from './model';

const make = (i: RaiseInput): Notice => raise([], i, 1).notice as Notice;
const remote = (id: string, read = false) => ({
	...make({ key: `remote:${id}`, kind: 'remote', data: { title: id } }),
	read
});
const EMPTY: PublishedMemory = { read: [], dismissed: [] };

describe('what the center remembers of published notices', () => {
	it('reads back what it wrote, and anything else as nothing', () => {
		const m = { read: ['a'], dismissed: ['b'] };
		expect(parseMemory(serializeMemory(m))).toEqual(m);
		for (const raw of [null, '', '{', '[]', '{"v":2}', '{"v":1,"read":"a"}']) {
			expect(parseMemory(raw)).toEqual(EMPTY);
		}
		expect(parseMemory('{"v":1,"read":["a",3],"dismissed":[null,"b"]}')).toEqual({
			read: ['a'],
			dismissed: ['b']
		});
	});

	it('remembers a published notice read, and one dismissed', () => {
		const before = [remote('a'), remote('b'), remote('c')];
		const after = [remote('a', true), remote('c')];
		expect(remember(EMPTY, before, after)).toEqual({ read: ['a'], dismissed: ['b'] });
	});

	it('ignores the rows of every other kind', () => {
		const failed = make({ key: 'f', kind: 'download-failed' });
		expect(remember(EMPTY, [failed], [])).toEqual(EMPTY);
		expect(remember(EMPTY, [], [{ ...failed, read: true }])).toEqual(EMPTY);
	});

	it('does not repeat an id, and keeps the newest when it is full', () => {
		let m: PublishedMemory = { read: ['a'], dismissed: [] };
		m = remember(m, [], [remote('a', true)]);
		expect(m.read).toEqual(['a']);
		const many = Array.from({ length: MAX_REMEMBERED + 5 }, (_, i) => remote(`n${i}`, true));
		m = remember(EMPTY, [], many);
		expect(m.read).toHaveLength(MAX_REMEMBERED);
		expect(m.read.at(-1)).toBe(`n${MAX_REMEMBERED + 4}`);
	});
});

describe('keeping what is still published', () => {
	it('keeps a notice read early, while it is still published, however many come after', () => {
		let m: PublishedMemory = remember(EMPTY, [], [remote('early', true)]);
		for (let i = 0; i < MAX_REMEMBERED + 5; i++) {
			m = remember(m, [], [remote('early', true), remote(`n${i}`, true)]);
		}
		expect(m.read).toContain('early');
	});

	it('keeps a notice dismissed early while the file still publishes it', () => {
		let m: PublishedMemory = { read: [], dismissed: ['early'] };
		const many = Array.from({ length: MAX_REMEMBERED + 5 }, (_, i) => remote(`n${i}`));
		m = keepPublished(m, ['early']);
		m = remember(m, many, []);
		m = keepPublished(m, ['early']);
		expect(m.dismissed).toContain('early');
		expect(m.dismissed).toHaveLength(MAX_REMEMBERED);
	});
});
