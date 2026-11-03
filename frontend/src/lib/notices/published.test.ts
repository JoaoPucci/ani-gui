import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	parsePublished,
	publishedChanges,
	publishedKey,
	sourceNote,
	withSourceNote,
	type Published
} from './published';
import type { SourceChange } from './sources';

const item = (over: Record<string, unknown> = {}) => ({
	id: 'maint',
	title: 'Maintenance tonight',
	body: 'Back by 22:00.',
	link: 'https://github.com/JoaoPucci/ani-gui',
	priority: 'warning',
	about_source: false,
	...over
});

describe("the backend's answer", () => {
	it('reads the donation number and every well-formed notice', () => {
		const got = parsePublished({ donation_ask: 3, items: [item()] });
		expect(got.donationAsk).toBe(3);
		expect(got.items).toEqual<Published[]>([
			{
				id: 'maint',
				title: 'Maintenance tonight',
				body: 'Back by 22:00.',
				link: 'https://github.com/JoaoPucci/ani-gui',
				priority: 'warning',
				aboutSource: false
			}
		]);
	});

	it('takes an absent body or link as none, and an absent priority as info', () => {
		const got = parsePublished({
			donation_ask: null,
			items: [{ id: 'a', title: 'A', body: null, link: null }]
		});
		expect(got.items[0]).toMatchObject({ body: null, link: null, priority: 'info' });
		expect(got.donationAsk).toBeNull();
	});

	it('drops a notice whose link is not plain http or https, or whose fields are not text', () => {
		const got = parsePublished({
			items: [
				item({ id: 'js', link: 'javascript:alert(1)' }),
				item({ id: 'file', link: 'file:///etc/passwd' }),
				item({ id: 'creds', link: 'https://user:pw@example.org/' }),
				item({ id: 'junk', link: 'not a url' }),
				item({ id: 'num', title: 7 }),
				item({ id: 'empty', title: '' }),
				item({ id: 'prio', priority: 'shout' }),
				item({ id: '' }),
				'nope',
				item({ id: 'ok' })
			]
		});
		expect(got.items.map((n) => n.id)).toEqual(['ok']);
	});

	it('keeps only the first of two notices with one id', () => {
		const got = parsePublished({ items: [item({ title: 'First' }), item({ title: 'Second' })] });
		expect(got.items.map((n) => n.title)).toEqual(['First']);
	});

	it('reads anything else as nothing published', () => {
		for (const raw of [null, undefined, 'x', 3, [], { items: 'x' }, { donation_ask: -1 }]) {
			const got = parsePublished(raw);
			expect(got.items).toEqual([]);
			expect(got.donationAsk).toBeNull();
		}
		expect(parsePublished({ donation_ask: 1.5 }).donationAsk).toBeNull();
	});

	it('never passes on a link that is not http or https, whatever it is sent', () => {
		fc.assert(
			fc.property(fc.string(), (link) => {
				for (const n of parsePublished({ items: [item({ link })] }).items) {
					expect(['http:', 'https:']).toContain(new URL(n.link as string).protocol);
				}
			})
		);
	});
});

const pub = (over: Partial<Published> = {}): Published => ({
	id: 'maint',
	title: 'Maintenance tonight',
	body: 'Back by 22:00.',
	link: 'https://example.org/m',
	priority: 'warning',
	aboutSource: false,
	...over
});

describe('the rows published notices raise', () => {
	it('raises one row per notice, keyed by its id, carrying its fields', () => {
		const changes = publishedChanges([pub()], { read: [], dismissed: [] });
		expect(changes).toEqual<SourceChange[]>([
			{
				op: 'raise',
				input: {
					key: publishedKey('maint'),
					kind: 'remote',
					read: false,
					data: {
						title: 'Maintenance tonight',
						body: 'Back by 22:00.',
						link: 'https://example.org/m',
						priority: 'warning'
					}
				}
			}
		]);
	});

	it('raises a notice read at an earlier launch as read, and leaves out one dismissed', () => {
		const changes = publishedChanges([pub({ id: 'a' }), pub({ id: 'b' }), pub({ id: 'c' })], {
			read: ['a'],
			dismissed: ['b']
		});
		expect(changes.map((c) => (c.op === 'raise' ? [c.input.key, c.input.read] : null))).toEqual([
			['remote:a', true],
			['remote:c', false]
		]);
	});

	it('leaves a notice about the source out of the list: it joins the outage row', () => {
		expect(publishedChanges([pub({ aboutSource: true })], { read: [], dismissed: [] })).toEqual([]);
	});
});

describe('a notice about the source', () => {
	const outage: SourceChange = {
		op: 'raise',
		input: { key: 'outage:hianime', kind: 'outage', count: 0, data: { since: 1 } }
	};
	const other: SourceChange = { op: 'resolve', key: 'outage:hianime', data: {} };

	it('is the first notice published about the source', () => {
		expect(sourceNote([pub({ id: 'x' }), pub({ id: 'y', aboutSource: true })])).toEqual({
			title: 'Maintenance tonight',
			body: 'Back by 22:00.',
			link: 'https://example.org/m'
		});
		expect(sourceNote([pub()])).toBeNull();
	});

	it("joins the outage row's data, and leaves every other change as it was", () => {
		const note = { title: 'Down for maintenance', body: null, link: null };
		expect(withSourceNote([outage, other], note)).toEqual([
			{
				op: 'raise',
				input: {
					key: 'outage:hianime',
					kind: 'outage',
					count: 0,
					data: { since: 1, noteTitle: 'Down for maintenance', noteBody: null, noteLink: null }
				}
			},
			other
		]);
		expect(withSourceNote([outage], null)).toEqual([outage]);
	});
});
