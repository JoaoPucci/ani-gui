import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	downloadChanges,
	outageChanges,
	outageKey,
	parseHealth,
	updateChange,
	type DownloadOutcome
} from './sources';

describe('the update row', () => {
	it('is raised while a newer release exists and withdrawn after', () => {
		expect(updateChange('v0.20.1', '0.20.0')).toEqual({
			op: 'raise',
			input: { key: 'update', kind: 'update', data: { tag: 'v0.20.1', current: '0.20.0' } }
		});
		expect(updateChange(null, '0.20.0')).toEqual({ op: 'withdraw', key: 'update' });
	});
});

describe('the source health answer', () => {
	it('reads each provider state', () => {
		const body = {
			providers: [
				{ provider: 'hianime', state: 'answering' },
				{
					provider: 'a',
					state: 'not_answering',
					since_ms: 10,
					failures: 4,
					last_failure_ms: 40
				},
				{ provider: 'b', state: 'answering_again', since_ms: 10, recovered_ms: 90 }
			]
		};
		expect(parseHealth(body)).toEqual(body.providers);
	});

	it('leaves out anything that is not a provider answer', () => {
		expect(parseHealth(null)).toEqual([]);
		expect(parseHealth({ providers: 'x' })).toEqual([]);
		expect(
			parseHealth({
				providers: [
					null,
					7,
					{ provider: 'Bad Name', state: 'answering' },
					{ provider: 'a', state: 'down' },
					{ provider: 'a', state: 'not_answering', since_ms: -1, failures: 4, last_failure_ms: 1 },
					{ provider: 'a', state: 'not_answering', since_ms: 1, failures: 'x', last_failure_ms: 1 },
					{ provider: 'a', state: 'answering_again', since_ms: 1 },
					{ provider: 'a', state: 'answering_again', since_ms: NaN, recovered_ms: 1 }
				]
			})
		).toEqual([]);
	});

	it('never throws on any value', () => {
		fc.assert(
			fc.property(fc.anything(), (v) => {
				expect(Array.isArray(parseHealth(v))).toBe(true);
				expect(Array.isArray(parseHealth({ providers: [v] }))).toBe(true);
			})
		);
	});
});

describe('outage rows from the health answer', () => {
	it('raises an outage while a provider is not answering, resolves it once it answers again', () => {
		expect(
			outageChanges([
				{ provider: 'x', state: 'answering' },
				{
					provider: 'hianime',
					state: 'not_answering',
					since_ms: 10,
					failures: 4,
					last_failure_ms: 40
				},
				{ provider: 'b', state: 'answering_again', since_ms: 10, recovered_ms: 90 }
			])
		).toEqual([
			{ op: 'resolve', key: 'outage:x', data: {} },
			{
				op: 'raise',
				input: {
					key: 'outage:hianime',
					kind: 'outage',
					count: 0,
					data: { since: 10, failures: 4, last: 40 }
				}
			},
			{ op: 'resolve', key: 'outage:b', at: 90, data: { since: 10 } }
		]);
	});

	it('resolves a live outage when the provider simply answers, as after the backend restarts', () => {
		expect(outageChanges([{ provider: 'x', state: 'answering' }])).toEqual([
			{ op: 'resolve', key: 'outage:x', data: {} }
		]);
		expect(outageKey('x')).toBe('outage:x');
	});
});

describe('download rows', () => {
	const item = (o: Partial<DownloadOutcome>): DownloadOutcome => ({
		id: 'dl-1',
		title: 'Frieren',
		episode: '5',
		status: 'active',
		destDir: '/home/u/Videos',
		error: null,
		startedAt: 100,
		rangeTotal: null,
		...o
	});

	it('raises a row for each download that finished or failed, once', () => {
		const seen = new Set<string>();
		const items = [
			item({ id: 'a', status: 'done' }),
			item({ id: 'b', status: 'done', rangeTotal: 3 }),
			item({ id: 'c', status: 'error', error: 'The source stopped answering.' }),
			item({ id: 'd', status: 'active' }),
			item({ id: 'e', status: 'queued' })
		];
		expect(downloadChanges(items, seen)).toEqual([
			{
				op: 'raise',
				input: {
					key: 'download-done:Frieren',
					kind: 'download-done',
					count: 1,
					data: { title: 'Frieren', destDir: '/home/u/Videos' }
				}
			},
			{
				op: 'raise',
				input: {
					key: 'download-done:Frieren',
					kind: 'download-done',
					count: 3,
					data: { title: 'Frieren', destDir: '/home/u/Videos' }
				}
			},
			{
				op: 'raise',
				input: {
					key: 'download-failed:100:c',
					kind: 'download-failed',
					data: { title: 'Frieren', episode: '5', reason: 'The source stopped answering.' }
				}
			}
		]);
		expect([...seen]).toEqual(['a', 'b', 'c']);
		expect(downloadChanges(items, seen)).toEqual([]);
	});
});
