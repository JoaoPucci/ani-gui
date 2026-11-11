import { describe, expect, it, vi } from 'vitest';
import { startPublishedRead } from './published-read';
import type { PublishedMemory } from './published-memory';
import type { SourceChange } from './sources';

const ANSWER = {
	donation_ask: 4,
	items: [
		{
			id: 'maint',
			title: 'Maintenance tonight',
			body: null,
			link: null,
			priority: 'warning',
			about_source: false
		}
	]
};
const NOTE_CHANGE: SourceChange = { op: 'resolve', key: 'outage:hianime', data: {} };

function deferred() {
	let resolve!: (v: unknown) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<unknown>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

function deps(
	read: () => Promise<unknown>,
	remembered: PublishedMemory = { read: [], dismissed: [] }
) {
	const log: string[] = [];
	return {
		log,
		read,
		keepPublished: vi.fn((ids: readonly string[]) => {
			log.push(`keep ${ids.join(',')}`);
			return remembered;
		}),
		joinNote: vi.fn(() => {
			log.push('note');
			return [NOTE_CHANGE];
		}),
		take: vi.fn((c: readonly SourceChange[]) => {
			log.push(`take ${c.map((x) => (x.op === 'raise' ? x.input.key : x.key)).join(',')}`);
		}),
		setNumber: vi.fn((n: number | null) => void log.push(`ask ${n}`))
	};
}
const flush = () => new Promise((r) => setTimeout(r, 0));

describe('reading what the maintainer published, once', () => {
	it('works out the note first, then remembers, raises, asks, and joins the note last', async () => {
		const d = deps(async () => ANSWER, { read: ['maint'], dismissed: [] });
		startPublishedRead(d);
		await flush();
		expect(d.log).toEqual([
			'note',
			'keep maint',
			'take remote:maint',
			'ask 4',
			'take outage:hianime'
		]);
		expect(d.joinNote).toHaveBeenCalledWith([expect.objectContaining({ id: 'maint' })]);
		const raised = d.take.mock.calls[0][0][0];
		expect(raised.op === 'raise' && raised.input.read).toBe(true);
	});

	it('leaves out a notice dismissed at an earlier launch', async () => {
		const d = deps(async () => ANSWER, { read: [], dismissed: ['maint'] });
		startPublishedRead(d);
		await flush();
		expect(d.take.mock.calls[0][0]).toEqual([]);
	});

	it('reads a failed request as nothing published', async () => {
		const d = deps(() => Promise.reject(new Error('offline')));
		startPublishedRead(d);
		await flush();
		expect(d.log).toEqual(['note', 'keep ', 'take ', 'ask null', 'take outage:hianime']);
		expect(d.joinNote).toHaveBeenCalledWith([]);
	});

	it('does nothing with an answer that arrives after it was stopped', async () => {
		const answer = deferred();
		const d = deps(() => answer.promise);
		const stop = startPublishedRead(d);
		stop();
		answer.resolve(ANSWER);
		await flush();
		expect(d.log).toEqual([]);
	});

	it('does nothing with a failure that arrives after it was stopped', async () => {
		const answer = deferred();
		const d = deps(() => answer.promise);
		const stop = startPublishedRead(d);
		stop();
		answer.reject(new Error('offline'));
		await flush();
		expect(d.log).toEqual([]);
	});
});
